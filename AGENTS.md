The goal is to be CSS-spec compliant.

After doing changes, always run tests.

When writing code, please first write a test (test driven development), and then
try to fix/implement a feature.

When fixing a bug, or adding features, make sure to add tests for these.

When doing changes, please also see if offscreen rendering outputs correct result.
cargo run --bin offscreen_render -- ui_demo.html /tmp/render.svg <width> <height>
When visual output doesn't match expectations but tests somehow pass, analyze why tests didn't catch the regression/bug.
