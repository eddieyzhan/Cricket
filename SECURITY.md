# Security model

Cricket is an early preview. Report a sensitive issue privately using GitHub's private vulnerability reporting if enabled, or contact the repository owner before publishing transfer codes or credentials in an issue. Never include `state.json`, `agent.json`, or an unredacted private-chat issue in a public bug report.

## Trust boundaries

- croc encrypts file data in transit. Cricket uses the official croc 11.5.3 binary and its relay protocol. The relay can observe connection metadata, not plaintext file contents protected by croc.
- GitHub hosts private chat metadata, including filenames and live croc bearer codes. GitHub and **every member of that chat** can read those codes and potentially compete to receive a transfer. The per-recipient label is a workflow boundary, not cryptographic recipient isolation. Do not put mutually untrusted people in a chat.
- GitHub owners/collaborators can modify manifests and history. Receipts are author-validated status records, not tamper-proof signatures or proof that a human opened a file.
- OAuth/classic `repo` authorization is broad. The application checks repository privacy and only accesses Cricket chats in its normal workflow, but the credential itself may grant other repository access. It is stored in the OS vault and never sent to the renderer.
- The local agent token grants the same file-transfer authority as the app. It is only available in the signed-in user's application-data directory. The API binds to `127.0.0.1` with a random port, rejects browser Origin headers, and requires a new random bearer token on every launch. It is not a network sharing API.

## Defensive behavior

Repository and login inputs are constrained before constructing API paths. Wire data has size/schema checks; issue/comment authors are checked against the protocol. Invalid data is ignored. Shell execution is never used for transfers. There is no arbitrary command endpoint. croc bearer secrets are environment variables; raw subprocess output is discarded. A separate folder is created for each receive attempt. Existing downloads are not overwritten automatically. Remote metadata cannot trigger an automatic send, retry, download, or execution.

Local history contains metadata and saved source paths and must be treated as private. Unix files are created with `0600`; Windows relies on the user's application-data ACL. Local state is not additionally encrypted. File contents remain wherever the user selected them or downloaded them. Disconnect removes the stored GitHub credential and local chat history after active transfers/pending receipt writes are resolved; it does not delete GitHub repositories or downloaded files.

There is no telemetry, analytics endpoint, automated paid infrastructure, or automatic updater in this preview. Code signing, OS-by-OS validation, and broader hostile-input testing remain necessary before a stable release.
