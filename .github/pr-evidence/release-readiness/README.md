# Chiviz release readiness evidence

`test-results.png` is a rendered excerpt from the captured Python launcher test
output in `python-tests.txt`; `test-results.txt` provides the text summary. Both
are test evidence, not product UI screenshots. These paths are relative to this
directory.
The fake `PATH` tests use temporary directories and stub executables; they do
not write to a real `~/.local/bin` or Omarchy configuration.

Reproduce the checks from the repository root:

```bash
python3 -m unittest discover -s tests -v
/usr/bin/bash -n scripts/chiviz-launch
omarchy plugin validate .
/usr/lib/qt6/bin/qmllint -I /tmp/chiviz-qml-imports BarWidget.qml
CARGO_TARGET_DIR=/tmp/omarchy-review-chiviz-target cargo test --locked --offline
```

For QML lint, create the temporary import link once:

```bash
mkdir -p /tmp/chiviz-qml-imports
ln -sfn /usr/share/omarchy/shell /tmp/chiviz-qml-imports/qs
```

`install-flow.svg` diagrams the documented build-before-enable sequence and
the three widget routes. `shellcheck` was not installed in the validation
environment; Bash syntax was checked with `bash -n`. `qmllint` exits 0 and
reports the pre-existing `Style.font.body` singleton type warning (the
installed Omarchy `Style.qml` defines this property) plus two unused-import
info messages.
