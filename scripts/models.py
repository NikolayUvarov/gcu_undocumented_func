#!/usr/bin/env python3
"""The model cache: speech models pinned in models/manifest.toml, kept on the host and carried as a pack or a disk.

  models.py list    [selection]            what the manifest names, which variant, licence, size, whether it is cached
  models.py fetch   [selection] [--from S]  download missing files (or copy them from a cache, pack or mounted disk S)
  models.py verify  [selection]             hash every cached file again
  models.py pack    OUT.tar [selection]     one file to copy elsewhere; `fetch --from OUT.tar` takes it back
  models.py disk    OUT.img [selection] [--add PATH=FILE]  a FAT32 disk of models for MIND Core (MANIFEST.json at its root)
  models.py pin     REPO PATH... --id ID    print a manifest entry for files of a Hugging Face repository, hashed
                                           (a PATH ending in / takes every file below it)

A source is a Hugging Face repository at a revision, or a zip or tar archive (url, sha256, size, strip: the prefix of its
paths).
A model may serve several variants (`variant` a list), name the voices chosen from it (`voices`), and need other models
(`needs`, such as a vocoder): a selection takes those along.

A selection is any of --variant compact|quality, --role asr|tts, --lang ru|en and model ids; none means every model.
The cache is $MIND_MODELS, else ~/.cache/mind-models: <cache>/<id>/<path> and <cache>/MANIFEST.json. Every file is
checked against its SHA-256 before it is used, whatever it came from.
"""
import argparse
import hashlib
import json
import os
import shutil
import sys
import tarfile
import tempfile
import tomllib
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "models" / "manifest.toml"
CHUNK = 1 << 20


def cache_dir(arg):
    return Path(arg or os.environ.get("MIND_MODELS") or Path.home() / ".cache" / "mind-models").expanduser()


def load_manifest():
    with open(MANIFEST, "rb") as f:
        data = tomllib.load(f)
    models = data.get("model", [])
    ids = [m["id"] for m in models]
    if len(ids) != len(set(ids)):
        raise ValueError("manifest: duplicate model ids")
    return models


def select(models, args):
    picked = []
    for m in models:
        if args.ids and m["id"] not in args.ids:
            continue
        if args.variant and args.variant not in variants(m):
            continue
        if args.role and m["role"] != args.role:
            continue
        if args.lang and args.lang not in m["lang"]:
            continue
        picked.append(m)
    unknown = set(args.ids or ()) - {m["id"] for m in models}
    if unknown:
        raise ValueError(f"not in the manifest: {', '.join(sorted(unknown))}")
    by_id, ids = {m["id"]: m for m in models}, [m["id"] for m in picked]
    for model_id in ids:  # what a picked model needs comes along, after it
        for need in by_id[model_id].get("needs", []):
            if need not in by_id:
                raise ValueError(f"{model_id} needs {need}, which is not in the manifest")
            if need not in ids:
                ids.append(need)
    return [by_id[i] for i in ids]


def variants(model):
    v = model["variant"]
    return v if isinstance(v, list) else [v]


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(CHUNK), b""):
            h.update(chunk)
    return h.hexdigest()


def url_of(model, path):
    src = model["source"]
    if src["kind"] == "huggingface":
        return f"https://huggingface.co/{src['repo']}/resolve/{src['revision']}/{path}"
    if src["kind"] == "url":
        return src["base"].rstrip("/") + "/" + path
    raise ValueError(f"{model['id']}: unknown source kind {src['kind']}")


def download(url, target, expected):
    """Stream `url` into `target` through a partial file, keeping it only if its SHA-256 is `expected`."""
    partial = target.with_name(target.name + ".partial")
    partial.parent.mkdir(parents=True, exist_ok=True)
    h = hashlib.sha256()
    with urllib.request.urlopen(url, timeout=60) as r, open(partial, "wb") as out:
        for chunk in iter(lambda: r.read(CHUNK), b""):
            h.update(chunk)
            out.write(chunk)
    if h.hexdigest() != expected:
        partial.unlink()
        raise ValueError(f"{url}: SHA-256 {h.hexdigest()}, the manifest says {expected}")
    os.replace(partial, target)


_ARCHIVES_CHECKED = set()  # archives hashed once in this run


def from_archive(cache, model, entry, target):
    """A file of a zip or tar archive the source names: the archive is fetched once, checked, and kept under
    <cache>/.archives."""
    src = model["source"]
    is_zip = src["url"].endswith(".zip")
    archive = cache / ".archives" / (src["sha256"] + (".zip" if is_zip else ".tar"))
    if archive not in _ARCHIVES_CHECKED:
        if not archive.is_file() or sha256_of(archive) != src["sha256"]:
            print(f"{model['id']}: archive {src['url']} ({src['size'] / 1e6:.1f} MB)", flush=True)
            download(src["url"], archive, src["sha256"])
        _ARCHIVES_CHECKED.add(archive)
    member = src.get("strip", "") + entry["path"]
    with tempfile.TemporaryDirectory(dir=cache) as tmp:
        extracted = Path(tmp) / "file"
        if is_zip:
            with zipfile.ZipFile(archive) as z, z.open(member) as zf, open(extracted, "wb") as out:
                shutil.copyfileobj(zf, out, CHUNK)
        else:
            with tarfile.open(archive) as t:
                tf = t.extractfile(member)
                if tf is None:
                    raise ValueError(f"{src['url']}: no file {member}")
                with tf, open(extracted, "wb") as out:
                    shutil.copyfileobj(tf, out, CHUNK)
        copy_checked(extracted, target, entry["sha256"])


def copy_checked(source, target, expected):
    if sha256_of(source) != expected:
        raise ValueError(f"{source}: SHA-256 differs from the manifest")
    partial = target.with_name(target.name + ".partial")
    partial.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, partial)
    os.replace(partial, target)


def write_cache_manifest(cache, models):
    """MANIFEST.json lists the manifest entries whose files are all in the cache."""
    present = [m for m in models if all((cache / m["id"] / f["path"]).is_file() for f in m["files"])]
    data = {"format": 1, "models": present}
    tmp = cache / "MANIFEST.json.partial"
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
    os.replace(tmp, cache / "MANIFEST.json")


def present(cache, model, check):
    for f in model["files"]:
        p = cache / model["id"] / f["path"]
        if not p.is_file() or p.stat().st_size != f["size"] or (check and sha256_of(p) != f["sha256"]):
            return False
    return True


def cmd_list(args, models):
    cache = cache_dir(args.cache)
    total = 0
    for m in select(models, args):
        size = sum(f["size"] for f in m["files"])
        total += size
        mark = "cached" if present(cache, m, False) else "-"
        print(f"{m['id']:34} {m['role']:3} {','.join(m['lang']):5} {','.join(variants(m)):15} {size / 1e6:8.1f} MB  {m['licence']:22} {mark}")
        for v in m.get("voices", []):
            print(f"{'':36}voice {v['name']} ({v['gender']}, {v['lang']}, {', '.join(v['variant'] if isinstance(v['variant'], list) else [v['variant']])})")
        if m.get("needs"):
            print(f"{'':36}needs {', '.join(m['needs'])}")
    print(f"{'':34} {'':3} {'':5} {'':15} {total / 1e6:8.1f} MB  in all")


def cmd_fetch(args, models):
    cache = cache_dir(args.cache)
    cache.mkdir(parents=True, exist_ok=True)
    source = Path(args.source) if args.source else None
    tar = tarfile.open(source) if source and source.is_file() else None
    try:
        for m in select(models, args):
            for f in m["files"]:
                target = cache / m["id"] / f["path"]
                if target.is_file() and target.stat().st_size == f["size"] and sha256_of(target) == f["sha256"]:
                    continue
                print(f"{m['id']}: {f['path']} ({f['size'] / 1e6:.1f} MB)", flush=True)
                if tar:
                    member = tar.getmember(f"{m['id']}/{f['path']}")
                    with tempfile.TemporaryDirectory(dir=cache) as tmp:
                        extracted = Path(tmp) / "file"
                        with tar.extractfile(member) as src, open(extracted, "wb") as out:
                            shutil.copyfileobj(src, out, CHUNK)
                        copy_checked(extracted, target, f["sha256"])
                elif source:
                    copy_checked(source / m["id"] / f["path"], target, f["sha256"])
                elif m["source"]["kind"] == "archive":
                    from_archive(cache, m, f, target)
                else:
                    download(url_of(m, f["path"]), target, f["sha256"])
    finally:
        if tar:
            tar.close()
    write_cache_manifest(cache, models)
    print(f"cache: {cache}")


def cmd_verify(args, models):
    cache = cache_dir(args.cache)
    bad = 0
    for m in select(models, args):
        for f in m["files"]:
            p = cache / m["id"] / f["path"]
            state = "missing" if not p.is_file() else "ok" if sha256_of(p) == f["sha256"] else "DIFFERS"
            if state != "ok":
                bad += 1
                print(f"{m['id']}/{f['path']}: {state}")
    print("all files match the manifest" if not bad else f"{bad} file(s) missing or different")
    return 1 if bad else 0


def selected_present(args, models):
    cache = cache_dir(args.cache)
    picked = select(models, args)
    missing = [m["id"] for m in picked if not present(cache, m, False)]
    if missing:
        raise ValueError(f"not cached (run fetch first): {', '.join(missing)}")
    return cache, picked


def cmd_pack(args, models):
    cache, picked = selected_present(args, models)
    out = Path(args.output)
    with tempfile.TemporaryDirectory() as tmp:
        manifest = Path(tmp) / "MANIFEST.json"
        manifest.write_text(json.dumps({"format": 1, "models": picked}, ensure_ascii=False, indent=1), encoding="utf-8")
        with tarfile.open(out, "w:gz" if out.name.endswith(".gz") else "w", format=tarfile.PAX_FORMAT) as tar:
            tar.add(manifest, "MANIFEST.json")
            for m in picked:
                for f in m["files"]:
                    tar.add(cache / m["id"] / f["path"], f"{m['id']}/{f['path']}")
    print(f"{out}: {len(picked)} model(s), {out.stat().st_size / 1e6:.1f} MB")


def cmd_disk(args, models):
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import fat32
    cache, picked = selected_present(args, models)
    out = Path(args.output)
    if out.exists() and not args.force:
        raise ValueError(f"{out} exists; --force overwrites it")
    # Files made from the models (the dictation engine's network files, 250), each with its hash.
    added = []
    for item in args.add:
        path, _, source = item.partition("=")
        if not path or not source or not Path(source).is_file():
            raise ValueError(f"--add {item}: PATH=FILE with an existing file")
        added.append({"path": path, "size": Path(source).stat().st_size, "sha256": sha256_of(Path(source)), "source": source})
    with tempfile.TemporaryDirectory() as tmp:
        manifest = Path(tmp) / "MANIFEST.json"
        listed = [{k: v for k, v in a.items() if k != "source"} for a in added]
        manifest.write_text(json.dumps({"format": 1, "models": picked, **({"added": listed} if listed else {})}, ensure_ascii=False, indent=1), encoding="utf-8")
        files = {"MANIFEST.json": manifest}
        files.update({a["path"]: Path(a["source"]) for a in added})
        for m in picked:
            for f in m["files"]:
                files[f"{m['id']}/{f['path']}"] = cache / m["id"] / f["path"]
        info = fat32.build(out, files, label="MIND MODELS", extra=args.extra * 1_000_000)
    print(f"{out}: {len(picked)} model(s), {info['bytes'] / 1e6:.1f} MB, {info['free_bytes'] / 1e6:.1f} MB free (FAT32, label MIND MODELS)")


def cmd_pin(args, _models):
    """An entry for the manifest: the repository's current revision and each file's size and SHA-256."""
    api = f"https://huggingface.co/api/models/{args.repo}"
    with urllib.request.urlopen(api, timeout=60) as r:
        revision = json.load(r)["sha"]
    tree = {}
    for folder in sorted({p.rstrip("/") if p.endswith("/") else p.rpartition("/")[0] for p in args.paths}):
        # One directory at a time: a large repository's whole tree comes in pages.
        with urllib.request.urlopen(f"{api}/tree/{revision}/{folder}?recursive=true".replace("//?", "?"), timeout=60) as r:
            tree.update({e["path"]: e for e in json.load(r) if e["type"] == "file"})
    lines = ["[[model]]", f'id = "{args.id}"', 'role = ""', 'lang = []', 'variant = ""', 'engine = ""', 'licence = ""',
             f'source = {{ kind = "huggingface", repo = "{args.repo}", revision = "{revision}" }}', "files = ["]
    paths = []
    for path in args.paths:
        below = sorted(p for p in tree if p.startswith(path)) if path.endswith("/") else [path]
        if not below or below[0] not in tree:
            raise ValueError(f"{args.repo}@{revision}: no file {path}")
        paths += below
    for path in paths:
        e = tree[path]
        if "lfs" in e:
            digest, size = e["lfs"]["oid"], e["lfs"]["size"]
        else:
            with urllib.request.urlopen(f"https://huggingface.co/{args.repo}/resolve/{revision}/{path}", timeout=60) as r:
                body = r.read()
            digest, size = hashlib.sha256(body).hexdigest(), len(body)
        lines.append(f'  {{ path = "{path}", size = {size}, sha256 = "{digest}" }},')
    lines.append("]")
    print("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--cache", help="the cache directory (default: $MIND_MODELS or ~/.cache/mind-models)")
    sub = parser.add_subparsers(dest="command", required=True)

    def selection(p):
        p.add_argument("ids", nargs="*", help="model ids (default: every model)")
        p.add_argument("--variant", choices=("compact", "quality"))
        p.add_argument("--role", choices=("asr", "tts"))
        p.add_argument("--lang", choices=("ru", "en"))

    selection(sub.add_parser("list"))
    p = sub.add_parser("fetch"); selection(p); p.add_argument("--from", dest="source", help="a cache directory, a pack or a mounted model disk")
    selection(sub.add_parser("verify"))
    p = sub.add_parser("pack"); p.add_argument("output"); selection(p)
    p = sub.add_parser("disk"); p.add_argument("output"); selection(p)
    p.add_argument("--extra", type=int, default=0, help="free space to leave on the disk, MB")
    p.add_argument("--add", action="append", default=[], metavar="PATH=FILE", help="also put FILE at PATH, its SHA-256 under \"added\" in MANIFEST.json (e.g. asr-ru-vosk-0.54/dictate.bin=dictate-ru.bin)")
    p.add_argument("--force", action="store_true")
    p = sub.add_parser("pin"); p.add_argument("repo"); p.add_argument("paths", nargs="+"); p.add_argument("--id", required=True)
    args = parser.parse_args()
    models = [] if args.command == "pin" else load_manifest()
    return {"list": cmd_list, "fetch": cmd_fetch, "verify": cmd_verify, "pack": cmd_pack, "disk": cmd_disk, "pin": cmd_pin}[args.command](args, models) or 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
