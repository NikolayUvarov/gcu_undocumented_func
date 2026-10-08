# hf_get.py <dest> <repo> [pattern...]: download a Hugging Face repository (or the files matching patterns) into <dest>.
import sys
from huggingface_hub import snapshot_download
dest, repo, patterns = sys.argv[1], sys.argv[2], sys.argv[3:] or None
print(snapshot_download(repo_id=repo, local_dir=dest, allow_patterns=patterns))
