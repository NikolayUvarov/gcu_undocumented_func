#!/usr/bin/env bash
# Each machine's settings kept apart by its identity and put on any new disk (801-LNK-0046): the writer's own functions,
# taken from scripts/build_and_write.sh by name, run on plain directories. `sudo mount` and `umount` are stand-ins that
# link the mount point to a directory, so no disk, mount or Windows is involved; every file is a dummy.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
for name in has_content rotate_kept keep_file refill_empty keep_effector_from put_effector_to keep_settings_from put_settings_to \
            use_machine machine_name_of machine_id_of disk_commit write_machine_record record_field list_machines choose_machine \
            old_stores_note read_old_disk save_machine_from keep_keystore_from policy_changes merge_policy put_keystore_to \
            wait_new_disk key_label log_label old_log_label machine_labels check_device_key set_key_aside; do
    eval "$(awk -v n="$name" '$0 ~ "^" n "\\(\\) \\{" { on = 1 } on { print } on && (/^}$/ || ($0 ~ "^" n "\\(\\) \\{.*\\}$")) { exit }' "$ROOT/scripts/build_and_write.sh")"
    declare -F "$name" > /dev/null || { echo "FAIL: $name not found in build_and_write.sh"; exit 1; }
done
SETTINGS_SKIP_DATA=(effector firmware wallpapers link wifitest.txt wifi-ofdm.txt wifijoin.txt)
SETTINGS_SKIP_SYSTEM=(keystore netpolicy effector)
KEPT_FILES=(wifi.kept device.key device.sealed device.locked wifi.country)
KEY_FILES=(device.key device.sealed device.locked)
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
HOME=$work/home MACHINES_KEEP=$work/home/.config/mind/machines
MACHINE_DIR="" KEEP_DIR="" POLICY_KEEP="" EFFECTOR_KEEP="" SETTINGS_KEEP="" READ_OLD=0
KEEP_WIFI=1 KEEP_EFFECTOR=1 KEEP_SETTINGS=1 WIFIJOIN=0 KEEP_WIFI_NAME=TestNet WIFIJOIN_CONF=$work/none
DISK=4 OLD_DISK=4 MACHINE="" OLD_LETTER="" NEW_KEY=0 DISK_KEY_LABEL=""
cd "$ROOT" # put_keystore_to takes the tree's tests/hardware/wifijoin.msh
fail=0
check() { if [[ $2 == "$3" ]]; then echo "ok   $1"; else echo "FAIL $1: wanted '$3', got '$2'"; fail=1; fi; }
put() { mkdir -p "$(dirname "$1")"; printf '%b' "$2" > "$1"; }
# The volumes WSL would mount, by drive letter; the stand-in links the mount point to one.
declare -A VOLUME=()
sudo() {
    case $1 in
        mount) rmdir "$5" && ln -s "${VOLUME[${4%:}]}" "$5" ;;
        umount) rm "$2" && mkdir "$2" ;;
        *) echo "unexpected sudo $*" >&2; return 1 ;;
    esac
}
# The log volume (MIND LOG) of each disk number, by drive letter, as Windows would name it.
declare -A LOG_OF=()
log_letter() { echo "${LOG_OF[$1]:-}"; }
# The read of the old disk as the writer runs it (read_old_disk || exit 1): its output in $work/out, its status in status.
read_disk() { if read_old_disk > "$work/out" 2>&1; then status=0; else status=1; fi; }
# Everything under a directory, names and bytes, to show it unchanged.
tree_sum() { (cd "$1" && find . -type f -print0 | sort -z | xargs -0 sha256sum 2> /dev/null | sha256sum | cut -c1-16); }
# A machine's disk ($1) with agent ID $2 and name $3: the agent, the Wi-Fi keys, a changed policy, settings, a MANIFEST.
machine_disk() {
    local d=$1
    put "$d/MANIFEST" "MIND-MANIFEST 1\nkey 0000\ncommit ${4:-0123456789abcdef0123456789abcdef01234567}\n"
    [[ -n $2 ]] && put "$d/system/effector/agent_id" "$2\r\n"
    put "$d/system/effector/machine" "CPU of $3, 16 GiB\n"
    put "$d/system/effector/journal" "journal-of-$3"
    put "$d/data/effector/effector.conf" "effector-config 1\nserver https://effector.invalid\nhostname mind-$3\n"
    put "$d/data/effector/policy" "policy-of-$3"
    put "$d/system/keystore/wifi.kept" "wifi-of-$3"; put "$d/system/keystore/device.key" "dummy-key-of-$3"; put "$d/system/keystore/wifi.country" "RU"
    put "$d/netpolicy.txt" "shipped line\n"; put "$d/system/netpolicy/netpolicy.txt" "shipped line\nnetcheck of-$3 icmp\n"
    put "$d/data/clock.conf" "zone-of-$3"; put "$d/data/machine/name" "mind-$3\n"
    put "$d/data/firmware/blob.bin" "firmware the writer copies itself"
}
MAC=mind_00000000000000aa DEXP=mind_00000000000000bb
machine_disk "$work/mac-old" "$MAC" mbp2
machine_disk "$work/dexp" "$DEXP" dexp 89abcdef0123456789abcdef0123456789abcdef
VOLUME[M]=$work/mac-old VOLUME[D]=$work/dexp
# The one store of before, which must stay as it was and unused.
put "$HOME/.config/mind/settings/data/clock.conf" "zone-of-the-last-disk-read"; put "$HOME/.config/mind/effector/system/agent_id" "mind_00000000000000ff"
old_store="$(tree_sum "$HOME/.config/mind/settings") $(tree_sum "$HOME/.config/mind/effector")"
disks="$(tree_sum "$work/mac-old") $(tree_sum "$work/dexp")"

# The identity a disk records.
check "a disk is named by its agent's ID, its line end taken off" "$(machine_id_of "$work/mac-old")" "$MAC"
put "$work/owner-only/data/effector/agent_id" "mind_00000000000000cc\n"
check "the owner's agent_id names a disk whose agent has not started" "$(machine_id_of "$work/owner-only")" mind_00000000000000cc
put "$work/ryzen/data/machine/name" "mind-ryzen\n"; put "$work/ryzen/system/effector/agent_id" "../not an id\n"
check "without a valid agent ID, the machine's name" "$(machine_id_of "$work/ryzen")" host-mind-ryzen
put "$work/nameless/data/clock.conf" "zone"; put "$work/nameless/data/machine/name" "mindcore\n"; put "$work/nameless/system/keystore/device.key" "dummy-key"
check "a disk with no agent and the default name names no machine" "$(machine_id_of "$work/nameless" || echo none)" none
check "the commit of a slot disk, slot by slot" "$(put "$work/slots/MIND/A/MANIFEST" "commit aaaaaaaaaaaabbbb\n"; put "$work/slots/MIND/B/MANIFEST" "commit cccccccccccc\n"; disk_commit "$work/slots")" "A:aaaaaaaaaaaa B:cccccccccccc"

# Two machines kept side by side: each old disk read into its own machine's directory.
MACHINE="" OLD_DISK=4 OLD_LETTER=M; read_disk
check "the Mac's disk read" "$status $MACHINE $READ_OLD" "0 $MAC 1"
MACHINE="" OLD_DISK=5 OLD_LETTER=D; read_disk
check "the DEXP's disk read" "$status $MACHINE $READ_OLD" "0 $DEXP 1"
check "both machines kept" "$(ls "$MACHINES_KEEP" | tr '\n' ' ')" "$MAC $DEXP "
check "the Mac's agent kept as its own" "$(tr -d '\r\n' < "$MACHINES_KEEP/$MAC/effector/system/agent_id") $(cat "$MACHINES_KEEP/$MAC/effector/data/policy")" "$MAC policy-of-mbp2"
check "the DEXP's agent kept as its own" "$(tr -d '\r\n' < "$MACHINES_KEEP/$DEXP/effector/system/agent_id") $(cat "$MACHINES_KEEP/$DEXP/effector/data/policy")" "$DEXP policy-of-dexp"
check "each one's Wi-Fi keys and settings its own" "$(cat "$MACHINES_KEEP/$MAC/keystore/device.key" "$MACHINES_KEEP/$DEXP/keystore/device.key") $(cat "$MACHINES_KEEP/$MAC/settings/data/clock.conf" "$MACHINES_KEEP/$DEXP/settings/data/clock.conf")" "dummy-key-of-mbp2dummy-key-of-dexp zone-of-mbp2zone-of-dexp"
check "each one's network policy as changed" "$(grep -h netcheck "$MACHINES_KEEP/$MAC/netpolicy/changed.txt" "$MACHINES_KEEP/$DEXP/netpolicy/changed.txt" | tr '\n' ' ')" "netcheck of-mbp2 icmp netcheck of-dexp icmp "
check "the Mac's machine.txt" "$(cat "$MACHINES_KEEP/$MAC/machine.txt" | grep -v '^saved' | tr '\n' ' ')" "id $MAC hostname mind-mbp2 print CPU of mbp2, 16 GiB commit 0123456789ab device-key - "
check "machine.txt says when" "$(grep -c '^saved 20[0-9][0-9]-[01][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]Z$' "$MACHINES_KEEP/$DEXP/machine.txt")" 1
check "the directories are the owner's alone" "$(stat -c %a "$MACHINES_KEEP" "$MACHINES_KEEP/$MAC" "$MACHINES_KEEP/$MAC/keystore" | tr '\n' ' ')$(find "$MACHINES_KEEP" -type f ! -perm 600 | wc -l)" "700 700 700 0"
check "what the writer copies itself is not kept" "$(find "$MACHINES_KEEP" -path '*firmware*' | wc -l)" 0
check "the disks read are left as they were" "$(tree_sum "$work/mac-old") $(tree_sum "$work/dexp")" "$disks"

# The Mac's disk read again: its directory is updated, the copy before kept per machine; the DEXP's stays.
dexp_kept=$(tree_sum "$MACHINES_KEEP/$DEXP")
put "$work/mac-old/data/clock.conf" "zone-of-mbp2-later"
MACHINE="" OLD_DISK=4 OLD_LETTER=M; read_disk
check "the machine's own directory updated" "$(cat "$MACHINES_KEEP/$MAC/settings/data/clock.conf") $(cat "$MACHINES_KEEP/$MAC/settings.prev/data/clock.conf")" "zone-of-mbp2-later zone-of-mbp2"
check "another machine's directory untouched" "$(tree_sum "$MACHINES_KEEP/$DEXP")" "$dexp_kept"

# A write onto a new device from one machine's directory: nothing read from the new stick, the Mac's put on it.
put "$work/stick/netpolicy.txt" "shipped line\nnew shipped line\n"; put "$work/stick/data/services.txt" "shipped services"
MACHINE=$MAC OLD_DISK=6 OLD_LETTER=""; read_disk
check "--machine with no old disk read goes on" "$status $MACHINE $SETTINGS_KEEP" "0 $MAC $MACHINES_KEEP/$MAC/settings"
check "it says whose settings go on" "$(grep -c "machine $MAC's settings (host name mind-mbp2, saved 20" "$work/out")" 1
put_settings_to "$work/stick" && put_effector_to "$work/stick" && put_keystore_to "$work/stick" S > /dev/null
check "the new stick has the Mac's agent" "$(tr -d '\r\n' < "$work/stick/system/effector/agent_id") $(cat "$work/stick/system/effector/journal") $(cat "$work/stick/data/effector/policy")" "$MAC journal-of-mbp2 policy-of-mbp2"
check "the new stick has the Mac's Wi-Fi keys and settings" "$(cat "$work/stick/system/keystore/device.key") $(cat "$work/stick/data/clock.conf") $(cat "$work/stick/data/machine/name")" "dummy-key-of-mbp2 zone-of-mbp2-later mind-mbp2"
check "its policy: the new image's lines and the Mac's change" "$(tr '\n' '|' < "$work/stick/system/netpolicy/netpolicy.txt")" "shipped line|new shipped line|netcheck of-mbp2 icmp|"
check "the new image's own files stay" "$(cat "$work/stick/data/services.txt")" "shipped services"
check "nothing of the DEXP on it" "$(grep -rl 'of-dexp\|dexp' "$work/stick" | wc -l)" 0

# The old disk read first, then a new device written (--old-disk): the old disk names the machine, the new one gets it.
VOLUME[O]=$work/mac-old
MACHINE="" OLD_DISK=4 OLD_LETTER=O; read_disk
put "$work/stick2/netpolicy.txt" "shipped line\n"
put_settings_to "$work/stick2" && put_effector_to "$work/stick2" && put_keystore_to "$work/stick2" S > /dev/null
check "the old disk's machine goes on the other device" "$status $MACHINE $(tr -d '\r\n' < "$work/stick2/system/effector/agent_id") $(cat "$work/stick2/system/keystore/wifi.kept")" "0 $MAC $MAC wifi-of-mbp2"

# No machine known: the write is refused, nothing saved, and --list-machines named.
kept=$(tree_sum "$MACHINES_KEEP")
MACHINE="" OLD_DISK=4 OLD_LETTER=""; read_disk
check "no old disk and no --machine: refused" "$status $(grep -c -- '--list-machines' "$work/out")" "1 1"
VOLUME[N]=$work/nameless
MACHINE="" OLD_DISK=4 OLD_LETTER=N; read_disk
check "an old disk naming no machine, no --machine: refused" "$status $(grep -c 'names no machine' "$work/out") $(grep -c -- '--list-machines' "$work/out")" "1 1 1"
MACHINE=mind_00000000000000ee OLD_DISK=4 OLD_LETTER=""; read_disk
check "--machine naming a machine not kept: refused" "$status $(grep -c "Nothing is kept for machine mind_00000000000000ee" "$work/out")" "1 1"
check "a refusal keeps nothing and changes nothing" "$(tree_sum "$MACHINES_KEEP") $(find "$MACHINES_KEEP" -path '*dummy-key*' | wc -l)" "$kept 0"

# A machine's directory that cannot be written: the old disk, which may be the disk written, is not erased.
put "$work/a-file" "not a directory"
MACHINE="" OLD_DISK=4 OLD_LETTER=M; MACHINES_KEEP=$work/a-file/machines read_disk
check "a save that fails stops the write" "$status $(grep -c 'NOT saved from disk 4 into .*: nothing is written' "$work/out")" "1 1"
MACHINES_KEEP=$HOME/.config/mind/machines

# --machine naming one machine while the disk is another's: that disk is not read, its machine's files stay.
MACHINE=$MAC OLD_DISK=5 OLD_LETTER=D; read_disk
check "a disk of another machine than --machine's is not read" "$status $READ_OLD $(grep -c "holds machine $DEXP's settings, not $MAC's" "$work/out")" "0 0 1"
check "and that machine's kept files stay as they were" "$(tree_sum "$MACHINES_KEEP")" "$kept"

# --list-machines: the ID, the host name, the date, the parts; never a file's content.
list_machines > "$work/list"
check "--list-machines lists both" "$(awk 'NR > 1 { print $1, $2, $4, $5 }' "$work/list" | tr '\n' '|')" "$MAC mind-mbp2 settings,effector,keystore,netpolicy -|$DEXP mind-dexp settings,effector,keystore,netpolicy -|"
check "--list-machines says when each was saved" "$(awk 'NR > 1 && $3 ~ /^20[0-9][0-9]-[01][0-9]-[0-3][0-9]T/' "$work/list" | wc -l)" 2
check "--list-machines prints no secret" "$(grep -c 'dummy-key\|wifi-of\|journal-of\|policy-of' "$work/list" || true)" 0
check "--list-machines with none kept" "$(MACHINES_KEEP=$work/empty list_machines)" "No machine's settings are kept in $work/empty yet: a write with --keep-settings (or --keep-wifi, --keep-effector) from a machine's own disk keeps them."

# The one store of before: named as left untouched and unused, and still as it was.
check "the one store of before is said to be unused" "$(old_stores_note | grep -c 'left untouched and unused')" 1
check "the one store of before is left as it was" "$(tree_sum "$HOME/.config/mind/settings") $(tree_sum "$HOME/.config/mind/effector")" "$old_store"
check "and nothing of it reached a machine" "$(grep -rl 'zone-of-the-last-disk-read\|mind_00000000000000ff' "$MACHINES_KEEP" | wc -l)" 0

# --old-disk's wait for the new device: disk_state is a stand-in playing back the states Windows would give, one a poll;
# no key is pressed and no time passes.
swap() {
    printf '%s\n' "$@" > "$work/states"; echo 0 > "$work/polls"
    ( disk_state() { local n; n=$(cat "$work/polls"); echo $((n + 1)) > "$work/polls"; sed -n "$((n + 1))p" "$work/states"; }
      read() { return 1; }; sleep() { :; }
      wait_new_disk ) > "$work/out" 2>&1 && echo 0 || echo 1
}
DISK=4 OLD_DISK=4 MACHINE=$MAC OLD_IDENT="USBSTOR_old|SN1|16008609792" OLD_DISK_WAIT=600
check "the old device still in, then out, then the new one: written" "$(swap old old absent "here USBSTOR_new|SN2|16008609792") $(cat "$work/polls")" "0 4"
check "it asks for the swap, and says when the old one is out" "$(grep -c 'Take it out now, and insert the NEW disk to be written as disk 4' "$work/out") $(grep -c 'The old disk is out, and there is no disk 4 yet' "$work/out") $(grep -c 'is the new disk, another device' "$work/out")" "1 1 1"
check "another device there at once: written with no question" "$(swap "here USBSTOR_new|SN2|16008609792") $(grep -c 'Take it out' "$work/out")" "0 0"
OLD_DISK_WAIT=0
check "the old device never taken out: nothing is written" "$(swap old) $(grep -c 'nothing is written' "$work/out")" "1 1"
OLD_IDENT=""
check "without the old device's identity, only Enter goes on" "$(swap "here USBSTOR_new|SN2|16008609792") $(grep -c 'nothing is written' "$work/out")" "1 1"
OLD_DISK_WAIT=600

# One device key on one machine. The keys are RFC 8032's public test vectors 1 and 2 (section 7.1), stored as keystore
# stores a seed (MINDKEY1, the seed, their SHA-256); their public keys start D75A9801 and 3D4017C3. The bench of
# 2026-11-14: the MacBook Pro's device.key and the DEXP's device.sealed, the same key sealed by its TPM.
# Bytes from hex, by bash's printf alone: gate_test runs these tests with python3 as a stand-in.
unhex() { printf "$(sed 's/../\\x&/g' <<< "$1")"; }
key_file() { local m="4d494e444b455931$2"; mkdir -p "$(dirname "$1")"; { unhex "$m"; unhex "$(unhex "$m" | sha256sum | cut -c1-64)"; } > "$1"; }
SEED1=9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60
SEED2=4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb
key_file "$work/k/one.key" $SEED1; key_file "$work/k/two.key" $SEED2
check "a device key's public label, as keystore logs it" "$(key_label "$work/k/one.key") $(key_label "$work/k/two.key")" "MIND D75A9801 MIND 3D4017C3"
cp "$work/k/one.key" "$work/k/bad.key"; printf 'X' | dd of="$work/k/bad.key" bs=1 seek=20 conv=notrunc status=none
check "a damaged key or another file has no label" "$(key_label "$work/k/bad.key" || echo none) $(key_label "$work/mac-old/system/keystore/device.key" || echo none)" "none none"
put "$work/log7/BOOT0001.LOG" "[KEYSTORE] DEVICE KEY READY: MIND 11111111 (ED25519, CERTIFICATE 300 BYTES)\r\n"
put "$work/log7/BOOT0002.LOG" "[INIT] START\r\n[KEYSTORE] DEVICE KEY FROM system/keystore/device.sealed (SEALED BY THE TPM)\r\n[KEYSTORE] DEVICE KEY READY: MIND D75A9801 (ED25519, CERTIFICATE 300 BYTES)\r\n"
put "$work/log7/README.TXT" "DEVICE KEY READY: MIND 22222222"
check "a sealed key's label from the newest boot log" "$(log_label "$work/log7")" "MIND D75A9801"
MACHINES_KEEP=$work/keys
MAC2=mind_00000000000000a1 DEXP2=mind_00000000000000b1 RYZEN2=mind_00000000000000c1 TPM2=mind_00000000000000d1
machine_disk "$work/mac2" "$MAC2" mbp2; key_file "$work/mac2/system/keystore/device.key" $SEED1
machine_disk "$work/dexp2" "$DEXP2" dexp; rm "$work/dexp2/system/keystore/device.key"; put "$work/dexp2/system/keystore/device.sealed" "sealed-blob-of-dexp"
machine_disk "$work/ryzen2" "$RYZEN2" ryzen; key_file "$work/ryzen2/system/keystore/device.key" $SEED2
machine_disk "$work/tpm2" "$TPM2" tpm; rm "$work/tpm2/system/keystore/device.key"; put "$work/tpm2/system/keystore/device.sealed" "sealed-blob-of-tpm"
VOLUME[K]=$work/mac2 VOLUME[E]=$work/dexp2 VOLUME[R]=$work/ryzen2 VOLUME[T]=$work/tpm2 VOLUME[L]=$work/log7
LOG_OF[7]=L
MACHINE="" OLD_DISK=6 OLD_LETTER=K; read_disk
check "the Mac's key recorded by its public label" "$status $(record_field "$MACHINES_KEEP/$MAC2/machine.txt" device-key)" "0 MIND D75A9801"
cp "$work/out" "$work/out-all"
MACHINE="" OLD_DISK=7 OLD_LETTER=E; read_disk
cat "$work/out" >> "$work/out-all"
check "the DEXP's sealed key kept, its label from its log" "$(cat "$MACHINES_KEEP/$DEXP2/keystore/device.sealed") $(record_field "$MACHINES_KEEP/$DEXP2/machine.txt" device-key)" "sealed-blob-of-dexp MIND D75A9801"
check "the same key on two machines: refused, both named" "$status $(grep -c "Machines $DEXP2 and $MAC2 hold the same device key (MIND D75A9801)" "$work/out") $(grep -c -- '--new-device-key' "$work/out")" "1 1 1"
MACHINE="" OLD_DISK=8 OLD_LETTER=R; read_disk
check "a machine with a key of its own goes on" "$status $(record_field "$MACHINES_KEEP/$RYZEN2/machine.txt" device-key)" "0 MIND 3D4017C3"
MACHINE=$MAC2 OLD_DISK=6 OLD_LETTER=""; read_disk
cat "$work/out" >> "$work/out-all"
check "the Mac written onto a new stick is refused too while the DEXP holds its key" "$status $(grep -c "Machines $MAC2 and $DEXP2 hold the same device key" "$work/out")" "1 1"
check "no key material in what the writer says" "$(grep -c "$SEED1\|${SEED1:0:16}\|9d 61 b1" "$work/out-all" || true) $(LC_ALL=C grep -c '[^[:print:][:space:]]' "$work/out-all" || true)" "0 0"
# --new-device-key for the DEXP: its sealed key and its networks set aside, none put on its new disk.
MACHINE=$DEXP2 OLD_DISK=7 OLD_LETTER="" NEW_KEY=1; read_disk; NEW_KEY=0
check "--new-device-key: the DEXP goes on" "$status $(record_field "$MACHINES_KEEP/$DEXP2/machine.txt" device-key) $(ls "$MACHINES_KEEP/$DEXP2/keystore" | grep -v '^replaced-' | tr '\n' ' ')" "0 - wifi.country "
check "its key and networks set aside, not removed" "$(cat "$MACHINES_KEEP/$DEXP2"/keystore/replaced-*/device.sealed) $(ls "$MACHINES_KEEP/$DEXP2"/keystore/replaced-*/ | tr '\n' ' ')" "sealed-blob-of-dexp device.sealed wifi.kept "
put "$work/stick3/netpolicy.txt" "shipped line\n"
put_keystore_to "$work/stick3" S > /dev/null 2>&1 || true
check "and nothing of the old key goes on its new disk" "$(ls "$work/stick3/system/keystore" | tr '\n' ' ')" "wifi.country "
MACHINE=$MAC2 OLD_DISK=6 OLD_LETTER=""; read_disk
check "then the Mac goes on with its key" "$status" 0
# A TPM machine's own sealed key goes back onto its own new disk.
LOG_OF[9]=U; VOLUME[U]=$work/log9; put "$work/log9/BOOT0007.LOG" "[KEYSTORE] DEVICE KEY READY: MIND 0BADC0DE (ED25519, CERTIFICATE 300 BYTES)\n"
MACHINE="" OLD_DISK=9 OLD_LETTER=T; read_disk
put "$work/stick4/netpolicy.txt" "shipped line\n"
put_keystore_to "$work/stick4" S > /dev/null 2>&1
check "a TPM machine's sealed key back on its own new disk" "$status $(cat "$work/stick4/system/keystore/device.sealed") $(cat "$work/stick4/system/keystore/wifi.kept")" "0 sealed-blob-of-tpm wifi-of-tpm"
unset 'LOG_OF[9]'; rm -rf "$MACHINES_KEEP/$TPM2"
MACHINE="" OLD_DISK=9 OLD_LETTER=T; read_disk
check "a sealed key with no label in a log: said, not refused" "$status $(grep -c "public label of machine $TPM2's sealed device key is not known" "$work/out")" "0 1"
MACHINES_KEEP=$HOME/.config/mind/machines

[[ $fail == 0 ]] && echo "build_and_write machines: all passed"
exit "$fail"
