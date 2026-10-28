import copy
import unittest

from service_events import CLOCK_NS, MODES, REQUESTS, verify_report


def fixture():
    pairs = []
    for rust_role in ["client", "server"]:
        for representation in ["typed", "untyped"]:
            pair = {"rust_role": rust_role, "representation": representation,
                    "service": "/oracle", "phases": []}
            for phase, mode in enumerate(MODES):
                calls = [{"a": phase * REQUESTS + i + 1, "b": 23,
                          "sum": phase * REQUESTS + i + 24, "sequence": phase * REQUESTS + i + 1}
                         for i in range(REQUESTS)]
                # Deliberately distinct identities model an RMW with different GID/writer encoding.
                client_gid, writer_guid = [11] * 16, [29] * 16
                events = []
                if mode != "off":
                    for call in calls:
                        for kind in range(4):
                            events.append({"info": {"event_type": kind, "sequence_number": call["sequence"],
                                                    "stamp": {"sec": 12, "nanosec": 34},
                                                    "client_gid": client_gid if kind in [0, 3] else writer_guid},
                                           "request": [{"a": call["a"], "b": 23}] if mode == "contents" and kind < 2 else [],
                                           "response": [{"sum": call["sum"]}] if mode == "contents" and kind >= 2 else []})
                record = {}
                for role in ["client", "server"]:
                    endpoint_calls = [dict(call, **({"client_gid": client_gid} if role == "client" else {"writer_guid": writer_guid}))
                                      for call in calls]
                    record[role] = {"kind": "phase", "implementation": "r2r" if role == rust_role else "rclcpp",
                                    "role": role, "representation": representation if role == rust_role else "cpp",
                                    "service": "/oracle", "phase": phase, "mode": mode, "rmw": "rmw_oracle",
                                    "clock_ns": CLOCK_NS, "event_publishers": 0 if mode == "off" else 2,
                                    "calls": endpoint_calls, "events": copy.deepcopy(events)}
                pair["phases"].append(record)
            pairs.append(pair)
    return {"metadata": {"ros_distro": "jazzy", "rmw": "rmw_oracle", "modes": MODES, "requests_per_phase": REQUESTS},
            "pairs": pairs, "summary": {"pairs": 4, "endpoint_phases": 40, "requests": 60, "observed_events": 288}}


class VerifyServiceEvents(unittest.TestCase):
    def test_native_identity_encodings_and_unordered_events_are_supported(self):
        report = fixture()
        for pair in report["pairs"]:
            for record in pair["phases"]:
                record["server"]["events"].reverse()
        verify_report(report)

    def reject(self, mutate):
        report = fixture()
        mutate(report)
        with self.assertRaises(ValueError):
            verify_report(report)

    def test_missing_or_duplicated_protocol_boundaries_are_rejected(self):
        self.reject(lambda r: r["pairs"].pop())
        self.reject(lambda r: r["pairs"][0]["phases"].pop())
        self.reject(lambda r: r["pairs"][0]["phases"][1]["client"]["events"].pop())
        self.reject(lambda r: r["pairs"][0]["phases"][1]["client"]["events"].__setitem__(0, copy.deepcopy(r["pairs"][0]["phases"][1]["client"]["events"][1])))

    def test_event_source_identity_timestamp_and_payload_are_checked(self):
        self.reject(lambda r: r["pairs"][0]["phases"][1]["client"]["events"][1]["info"].__setitem__("client_gid", [11] * 16))
        self.reject(lambda r: r["pairs"][0]["phases"][1]["client"]["events"][0]["info"]["stamp"].__setitem__("sec", 13))
        self.reject(lambda r: r["pairs"][0]["phases"][2]["client"]["events"][0]["request"][0].__setitem__("a", -1))
        self.reject(lambda r: r["pairs"][0]["phases"][1]["client"]["events"][0].__setitem__("request", [{"a": 4, "b": 23}]))

    def test_disabled_mode_and_reenable_are_not_false_greens(self):
        self.reject(lambda r: r["pairs"][0]["phases"][3]["server"]["events"].append(copy.deepcopy(r["pairs"][0]["phases"][2]["server"]["events"][0])))
        self.reject(lambda r: r["pairs"][0]["phases"][3]["server"].__setitem__("event_publishers", 1))
        self.reject(lambda r: r["pairs"][0]["phases"][4]["client"].__setitem__("events", []))

    def test_cross_endpoint_native_sequences_and_exact_calls_are_checked(self):
        self.reject(lambda r: r["pairs"][0]["phases"][0]["server"]["calls"][0].__setitem__("sequence", 100))
        self.reject(lambda r: r["pairs"][0]["phases"][0]["server"]["calls"][0].__setitem__("sum", 0))
        self.reject(lambda r: r["pairs"][0]["phases"][0]["server"]["calls"][0].__setitem__("writer_guid", [0] * 16))
        self.reject(lambda r: r["pairs"][0]["phases"][0]["client"].__setitem__("rmw", "rmw_other"))
        self.reject(lambda r: r["summary"].__setitem__("observed_events", 0))


if __name__ == "__main__":
    unittest.main()
