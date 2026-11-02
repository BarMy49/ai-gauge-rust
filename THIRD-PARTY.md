# Third-party components

AI Gauge application source is MIT licensed. The GUI uses Slint 1.18.1,
under the Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0.
The About dialog contains the required AboutSlint attribution widget.
Slint license texts are in licenses/.

The optional account helper is the official OpenAI Codex app-server, downloaded
on sign-in from github.com/openai/codex (Apache-2.0). It is not included in this
archive. It is used only for account authorization and quota reads.

Other Rust dependencies retain their respective licenses; exact resolved
packages are recorded in Cargo.lock, with license metadata available through
cargo metadata. No provider branding implies affiliation or endorsement.
