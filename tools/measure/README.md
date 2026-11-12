# Measuring Volsa on large scans

Scripts for Windows PowerShell 7 that measure how Volsa opens large made-up studies: how
long until the first slice and the 3D view, how much memory the program takes at its peak,
and how long pictures take to draw. Use made-up studies only, never a real scan.

## The studies

```text
cargo run --release -p volsa-app --example write_phantom -- ct target/big/ct
cargo run --release -p volsa-app --example write_phantom -- body target/big/body
cargo run --release -p volsa-app --example write_phantom -- zip target/big/ct target/big/ct.zip
cargo run --release -p volsa-app --example write_phantom -- zip target/big/ct target/big/ct-stored.zip stored
cargo run --release -p volsa-app --example write_phantom -- zip target/big/body target/big/body.zip
cargo build --release -p volsa-app --bin volsa --example measure_big
```

`ct` is a 512 x 512 CT of a made-up body in 2,000 slices (1 GB); `body` a whole-body MR in
five overlapping stations and the stations joined (832 slices). A head-sized study of
512 x 512 x 300 is looked for in `target/big/head300` (`-Head` names another folder).

## The scripts

- `sample.ps1 -Exe PATH -Arguments @(...) -Label NAME [-Env @{...}]` runs one program,
  samples its private memory every 25 ms and its graphics memory every 250 ms, and writes
  `target/measure/runs/NAME.{out,summary}.txt` and `NAME.samples.csv`.
- `repeat.ps1 -Prefix NAME -Runs 1,2,3` runs `measure_big` on each study on the fast card
  (`WGPU_POWER_PREF=high`) and the integrated one, and `volsa.exe` in its picture-and-quit mode
  (`VOLSA_CAPTURE`), through `sample.ps1`. `-NoApp` and `-NoIntel` leave those out.
- `summarise.ps1 -Prefix NAME` prints medians and ranges for each study.
- `cold.ps1 -Path STUDY -Label NAME` drops the study's files from Windows' file cache first,
  then measures one open.
- `warp_race.ps1 -Study PATH [-Runs 20]` starts `volsa.exe` on Windows' software graphics
  (`VOLSA_ADAPTER=software`) with the study opened at launch and resizes its window while it
  loads, and counts the runs Volsa's log says it stopped in. In 0.6.0, where a worker handed
  volumes to the device, three runs of three stopped here
  (`Surface::configure: GpuWaitTimeout`).
- `win.ps1` holds the window helpers the others use.

Busy machines give noisy numbers: note what else is running. A run that peaks at several
gigabytes pushes the study out of Windows' file cache, so the next run's first slice is
partly cold; repeat runs and take medians.
