# The public root store (000-NET-0046)

`cacert.pem` holds Mozilla's root certificates for TLS server authentication, as curl extracts them from Mozilla's NSS. The image builders ship it as `tlsroots.pem` at the boot volume's root. `scripts/update_roots.py` fetches a newer one, and `--check` checks this file against the record below.

- **Source:** https://curl.se/ca/cacert.pem (https://curl.se/docs/caextract.html)
- **Mozilla's data as of:** Tue Oct 20 13:32:40 2026 GMT
- **Roots:** 118
- **Bytes:** 200116
- **SHA-256:** `796f1389bfea13c0ed78f663b765baaff54a508e5483786bd6ece73c5e3e3cbe`
- **Licence:** MPL-2.0 (`LICENSES/MPL-2.0.txt`): curl's page says the file is "licensed under the same license as the Mozilla source file: MPL 2.0".
