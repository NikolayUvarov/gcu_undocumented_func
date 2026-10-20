# SSH client host tests

`cargo test --manifest-path ssh/Cargo.toml` runs the SSH client against OpenSSH on `127.0.0.1`, using temporary host and client keys, configuration and test files. Each server is stopped after its test. The tests require `ssh-keygen` and OpenSSH's server at `/usr/sbin/sshd`.

Set `MIND_SSHD` to another absolute executable path when using a separate test dependency. The same setting is checked by `scripts/ci_local.sh`; an unset or empty value retains the default. An invalid dependency fails the run, and all SSH assertions remain enabled.

On Ubuntu 24.04, an ordinary user with the server's shared-library dependencies can extract the official package without installing it or starting a system service:

```bash
mkdir -p /tmp/mind-openssh-test
cd /tmp/mind-openssh-test
apt-get download openssh-server
dpkg-deb --extract openssh-server*.deb root
ldd root/usr/sbin/sshd
export MIND_SSHD="$PWD/root/usr/sbin/sshd"
```

Run the tests or local gate from the repository with that environment. Use an unprivileged account; a root-launched server has additional privilege-separation account and directory requirements. The test process needs permission to bind loopback sockets. No change to the system's SSH configuration or service is needed, and neither the package nor its extracted files belongs in the repository.
