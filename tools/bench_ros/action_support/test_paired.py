import json
import os
from pathlib import Path
import tempfile
import unittest

from action_support.paired import paired_cohorts


STUB = '''#!/usr/bin/env python3
import json,os,sys,time
from pathlib import Path
directory=Path(os.environ["R2R_COHORT_CONTROL"])
label=os.environ["R2R_COHORT_LABEL"];role=sys.argv[1];count=int(sys.argv[6])
(directory/f"{label}-{role}.ready").write_text("")
for index in range(count):
 while not (directory/f"{label}-{index}.go").exists():time.sleep(.001)
 (directory/f"{label}-{index}-{role}.done").write_text("")
while not (directory/"render.go").exists():time.sleep(.001)
order=[p.name for p in sorted(directory.glob("*.go"),key=lambda p:p.stat().st_mtime_ns) if p.name!="render.go"]
print(json.dumps({"label":label,"role":role,"order":order,"ready":len(list(directory.glob("*.ready"))),"done":len(list(directory.glob("*.done")))}))
'''


class PairedCohortControl(unittest.TestCase):
    def test_every_cohort_alternates_and_all_endpoints_finish_before_reporting(self):
        cpus = sorted(os.sched_getaffinity(0))
        if len(cpus) < 2:
            self.skipTest("two CPUs required for endpoint affinity")
        with tempfile.TemporaryDirectory() as temporary:
            executable = Path(temporary) / "endpoint"
            executable.write_text(STUB); executable.chmod(0o755)
            for repetition in [0,1]:
                results=paired_cohorts(executable,executable,cpus[0],cpus[1],os.environ.copy(),0,10,5,4,repetition)
                expected=[]
                for index in range(4):
                    labels=["baseline","rust"] if (index+repetition)%2==0 else ["rust","baseline"]
                    expected.extend(f"{label}-{index}.go" for label in labels)
                for label,pair in results.items():
                    for role,result in zip(["ping","echo"],pair):
                        self.assertEqual((result["label"],result["role"]),(label,role))
                        self.assertEqual(result["order"],expected)
                        self.assertEqual((result["ready"],result["done"]),(4,16))

    def test_early_endpoint_failure_stops_and_cleans_up_other_endpoints(self):
        cpus=sorted(os.sched_getaffinity(0))
        if len(cpus)<2:self.skipTest("two CPUs required")
        with tempfile.TemporaryDirectory() as temporary:
            executable=Path(temporary)/"failed"
            executable.write_text("#!/bin/sh\nexit 9\n");executable.chmod(0o755)
            with self.assertRaisesRegex(RuntimeError,"initialization"):
                paired_cohorts(executable,executable,cpus[0],cpus[1],os.environ.copy(),0,10,5,4,0)


if __name__ == "__main__":
    unittest.main()
