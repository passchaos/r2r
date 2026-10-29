import copy
import unittest
from ros_sleep import validate, COMMANDS, TIMES, STATES


class RosSleepTraceBoundaries(unittest.TestCase):
    def test_trace_oracle_rejects_early_wakes_changed_domains_and_missing_cancellation(self):
        records = {language:{"ready":{"kind":"ready", "implementation":implementation},
            "trace":[{"command":command, "now_ns":time, "state":state} for command,time,state in zip(COMMANDS,TIMES,STATES)]}
            for language,implementation in [("rust","r2r"),("cpp","rclcpp")]}
        validate(records)
        for language in ["rust", "cpp"]:
            for index, field, value in [(3,"state","complete"),(4,"now_ns",200),(8,"state","pending")]:
                changed=copy.deepcopy(records); changed[language]["trace"][index][field]=value
                with self.assertRaisesRegex(RuntimeError, "trace mismatch"): validate(changed)
        missing=copy.deepcopy(records); del missing["cpp"]
        with self.assertRaisesRegex(RuntimeError, "missing ROS sleep"): validate(missing)


if __name__ == "__main__": unittest.main()
