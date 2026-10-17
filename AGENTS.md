# Repository working conventions

- Split implementation into focused commits, each with its relevant checks. Do not automatically push.
- On every build/delivery iteration, clean historical artifacts for the project being worked on. Keep the current delivery and its verification records, dependency/compiler caches, build tools and one pinned runtime cache. Do not routinely run `cargo clean`.
- Prepare a new delivery before removing the last working delivery. Failed builds retain that delivery and the current failure diagnostics.
- Before recursive removal or moving, resolve and validate absolute paths within the project's explicit artifact directories; reject reparse points. Never clean another project's outputs or user application data.
- Preserve user data found in old portable directories outside the artifact tree before removing those directories. Record cleanup decisions. Keep only selected compressed acceptance screenshots in Git; temporary QA applications are build artifacts.
