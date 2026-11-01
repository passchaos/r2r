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
API-shape-dependent resource comparison, not a leak or a timing claim.

The comparison is derived from the archived reusable-typed pipeline protocol reports. Timing
remains diagnostic on the contended host; RSS sampling does not affect the measured RTT protocol.

```sh
python3 tools/bench_ros/pipeline_resource_comparison.py \
  --cyclonedds docs/validation/reusable-typed-pipeline-2026-10-31/protocol/cyclonedds.json \
  --fastdds docs/validation/reusable-typed-pipeline-2026-10-31/protocol/fastdds.json \
  --output docs/validation/pipeline-resource-comparison-2026-11-01/comparison.json
python3 docs/validation/pipeline-resource-comparison-2026-11-01/verify.py
```
