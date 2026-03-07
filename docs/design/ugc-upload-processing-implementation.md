# UGC Upload & Processing (task-025)

Added UGC service contracts for upload chunks, validation, moderation, and artifact lifecycle transitions.

## Implemented API surface

- Added crate `aether-ugc` with modules:
  - `ingest`: chunk upload/session descriptors.
  - `validation`: file type/size/mime validation records.
  - `moderation`: moderation status and signal types.
  - `pipeline`: processing stages and content-address model.
  - `artifact`: lifecycle states and session descriptors.
- Updated workspace membership for `aether-ugc`.

## Mapping to acceptance criteria

- `#1` chunked upload represented by `UploadSession`, `ChunkUpload`, and request framing.
- `#2` upload validation via `FileValidation` and `ValidationReport`.
- `#3` moderation trigger represented via moderation signal and status updates.
- `#4` AOT profile abstraction via `AotProfile` and `UploaderProfile`.
- `#5` content-address model by `ContentAddress` and SHA fields on sessions/artifacts.
- `#6` manifest handoff represented by artifact descriptor/state progression.
- `#7` full lifecycle states in `ArtifactState` (upload → scan → approve → publish → archive).

## Remaining implementation work

- Add resumable upload APIs and storage backend bindings.
- Add compile pipeline execution and scan orchestration integrations.
