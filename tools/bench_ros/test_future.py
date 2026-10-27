import copy
import unittest
from future import gate, validate


class FutureBenchmarkValidation(unittest.TestCase):
    def setUp(self):
        self.row={'implementation':'r2r','rmw':'rmw_cyclonedds_cpp','iterations':3,'warmup':1,'delay_us':1000,
                  'completed':4,'checksum':6,'samples_ns':[10,20,30],'elapsed_ns':100,'completions_per_second':3/100e-9,
                  'notification_ns_p50':20,'notification_ns_p95':30,'notification_ns_p99':30}
    def test_complete_payload_timing_oracle(self):validate(self.row,'rmw_cyclonedds_cpp',3,1,1000)
    def test_count_sample_percentile_and_rate_errors_are_rejected(self):
        for field,value in [('completed',3),('checksum',5),('samples_ns',[10,20]),('notification_ns_p99',20),('elapsed_ns',50),('completions_per_second',1),('delay_us',2)]:
            row=copy.deepcopy(self.row);row[field]=value
            with self.subTest(field=field),self.assertRaises(RuntimeError):validate(row,'rmw_cyclonedds_cpp',3,1,1000)
    def test_latency_and_rate_gate_catches_each_boundary(self):
        baseline={'language':'baseline','notification_ns_p50':100,'notification_ns_p95':100,'notification_ns_p99':100,'completions_per_second':100}
        candidate=dict(baseline,language='candidate')
        self.assertTrue(gate([baseline,candidate],1.05)['passed'])
        for field,value in [('notification_ns_p50',106),('notification_ns_p95',106),('notification_ns_p99',106),('completions_per_second',94)]:
            with self.subTest(field=field):self.assertFalse(gate([baseline,dict(candidate,**{field:value})],1.05)['passed'])
        for maximum in [float('inf'),float('nan'),.99]:
            with self.assertRaises(RuntimeError):gate([baseline,candidate],maximum)


if __name__=='__main__':unittest.main()
