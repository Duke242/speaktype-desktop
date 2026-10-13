# Contributing to SpeakType 1

This folder holds SpeakType 1, the macOS-only Swift app. It only gets critical fixes. New features go into Wordfall, the cross-platform app at the root of this repository; its [contributing guide](../CONTRIBUTING.md) covers setup on every system.

## Set up

Requires macOS and a current Xcode. Run these from `macos-v1/`:

```bash
make setup   # first-time setup
make xcode   # open the project in Xcode
make run     # build and run
```

`make help` lists everything else, including installing, packaging and cleaning app data for a fresh start.

## Test your changes

```bash
make ci      # what CI runs: build, unit tests and SwiftLint
make test    # unit and UI tests
make format  # fix what SwiftLint can
```

## Make a pull request

1. Create a branch from `main`.
2. Keep the change focused, and describe what you tested.
3. Make sure `make ci` passes.
