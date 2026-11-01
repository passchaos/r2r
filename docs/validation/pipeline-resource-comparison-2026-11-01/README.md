# Independent-process pipeline peak RSS comparison (2026-11-01)

The pub/sub pipeline harness now samples each ping/echo process's `VmHWM` while the independent
Rust and rclcpp processes run the same three cases (0×1×1, 4096×16×1, 4096×8×4) over CycloneDDS and
Fast DDS. Median peak RSS in KiB, Rust/rclcpp:

| RMW | Case | Role | r2r | rclcpp | Ratio |
| --- | --- | --- | ---: | ---: | ---: |
| CycloneDDS | 0×1×1 | ping | 13,672 | 16,244 | 0.84 |
| CycloneDDS | 0×1×1 | echo | 13,584 | 16,096 | 0.84 |
| CycloneDDS | 4096×16×1 | ping | 19,544 | 18,400 | 1.06 |
| CycloneDDS | 4096×16×1 | echo | 19,528 | 18,226 | 1.07 |
| CycloneDDS | 4096×8×4 | ping | 19,274 | 18,124 | 1.06 |
| CycloneDDS | 4096×8×4 | echo | 30,642 | 19,002 | 1.61 |
| Fast DDS | 0×1×1 | ping | 23,496 | 26,456 | 0.89 |
| Fast DDS | 0×1×1 | echo | 23,716 | 26,618 | 0.89 |
| Fast DDS | 4096×16×1 | ping | 25,216 | 27,710 | 0.91 |
| Fast DDS | 4096×16×1 | echo | 25,112 | 27,912 | 0.90 |
| Fast DDS | 4096×8×4 | ping | 24,678 | 27,212 | 0.91 |
| Fast DDS | 4096×8×4 | echo | 26,118 | 28,796 | 0.91 |

r2r peak RSS is lower in eight of twelve comparisons. Under Fast DDS it is consistently about 9–11%
lower; under CycloneDDS it is 16% lower for the zero-payload case and up to 7% higher for the
4096-element cases. The CycloneDDS fanout echo is 61% higher because the r2r side runs the
`reusable_typed` receive path, which decodes every 4096-element payload into an owned Rust Vec,
while the rclcpp side validates native fields and republishes the native message. Four fanout
subscriptions retaining decoded payloads therefore dominate that process's peak RSS. This is an
API-shape-dependent resource comparison, not a leak.

The harness also records the user CPU time of each same-language ping/echo pair through
`RUSAGE_CHILDREN`, which is far less sensitive to host contention than wall-clock RTT:

| RMW | Case | r2r pair CPU | rclcpp pair CPU | Ratio |
| --- | --- | ---: | ---: | ---: |
| CycloneDDS | 0×1×1 | 11.16 ms | 18.39 ms | 0.61 |
| CycloneDDS | 4096×16×1 | 17.51 ms | 25.71 ms | 0.68 |
| CycloneDDS | 4096×8×4 | 18.47 ms | 16.16 ms | 1.14 |
| Fast DDS | 0×1×1 | 30.47 ms | 40.15 ms | 0.76 |
| Fast DDS | 4096×16×1 | 33.29 ms | 38.04 ms | 0.88 |
| Fast DDS | 4096×8×4 | 38.82 ms | 43.06 ms | 0.90 |

r2r uses 10–39% less pair CPU in five of six same-language cases. The exception is the CycloneDDS
fanout case, where the reusable typed echo's owned Rust decode costs 14% more CPU than the rclcpp
native validation path. Wall-clock RTT remains unqualified on the contended host.

The comparison is derived from the archived reusable-typed pipeline protocol reports. RSS and CPU
sampling do not alter the measured RTT protocol.

```sh
python3 tools/bench_ros/pipeline_resource_comparison.py \
  --cyclonedds docs/validation/reusable-typed-pipeline-2026-10-31/protocol/cyclonedds.json \
  --fastdds docs/validation/reusable-typed-pipeline-2026-10-31/protocol/fastdds.json \
  --output docs/validation/pipeline-resource-comparison-2026-11-01/comparison.json
python3 docs/validation/pipeline-resource-comparison-2026-11-01/verify.py
```
