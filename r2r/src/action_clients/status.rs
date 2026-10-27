use crate::{action_msgs, Error, GoalStatus, Result, WrappedTypesupport};

type NativeStatusArray = <action_msgs::msg::GoalStatusArray as WrappedTypesupport>::CStruct;

/// Extract only the client state updates, without allocating a Rust UUID vector per goal.
pub(super) fn decode_native(array: &NativeStatusArray) -> Result<Vec<(uuid::Uuid, GoalStatus)>> {
    let sequence = &array.status_list;
    if sequence.size == 0 {
        return Ok(Vec::new());
    }
    if sequence.data.is_null()
        || sequence.size > sequence.capacity
        || sequence.size
            > isize::MAX as usize / std::mem::size_of::<r2r_msg_gen::action_msgs__msg__GoalStatus>()
    {
        return Err(Error::RCL_RET_ERROR);
    }
    // This private boundary receives initialized, live ROS message storage from an owned
    // WrappedNativeMsg. Bounds are checked before creating the borrowed native member slice.
    let statuses = unsafe { std::slice::from_raw_parts(sequence.data, sequence.size) };
    Ok(statuses
        .iter()
        .map(|status| {
            (
                uuid::Uuid::from_bytes(status.goal_info.goal_id.uuid),
                GoalStatus::from_rcl(status.status),
            )
        })
        .collect())
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests {
    use super::*;
    use crate::{unique_identifier_msgs, NativeMsg};
    use std::time::Instant;

    fn fixture(count: usize) -> action_msgs::msg::GoalStatusArray {
        action_msgs::msg::GoalStatusArray {
            status_list: (0..count)
                .map(|index| action_msgs::msg::GoalStatus {
                    goal_info: action_msgs::msg::GoalInfo {
                        goal_id: unique_identifier_msgs::msg::UUID {
                            uuid: uuid::Uuid::from_u128(index as u128 + 1).as_bytes().to_vec(),
                        },
                        stamp: crate::builtin_interfaces::msg::Time {
                            sec: index as i32,
                            nanosec: 123,
                        },
                    },
                    status: [0, 1, 2, 3, 4, 5, 6, -1, 127][index % 9],
                })
                .collect(),
        }
    }

    #[test]
    fn native_status_decode_matches_full_rust_oracle_for_all_states() {
        for count in [0, 1, 9, 128, 1024] {
            let message = fixture(count);
            let native =
                NativeMsg::<action_msgs::msg::GoalStatusArray>::try_from(&message).unwrap();
            let reference = super::super::decode_status_updates(
                &action_msgs::msg::GoalStatusArray::from_native(&native),
            )
            .unwrap();
            assert_eq!(decode_native(&native).unwrap(), reference);
            assert_eq!(action_msgs::msg::GoalStatusArray::from_native(&native), message);
        }
    }

    #[test]
    fn native_status_decode_rejects_invalid_sequence_bounds_before_access() {
        let mut native = NativeMsg::<action_msgs::msg::GoalStatusArray>::try_new().unwrap();
        native.status_list.size = 1;
        assert!(matches!(decode_native(&native), Err(Error::RCL_RET_ERROR)));
        native.status_list.size = 0;
        let message = fixture(1);
        message.try_copy_to_native(&mut native).unwrap();
        let valid_size = native.status_list.size;
        native.status_list.size = native.status_list.capacity + 1;
        assert!(matches!(decode_native(&native), Err(Error::RCL_RET_ERROR)));
        let valid_capacity = native.status_list.capacity;
        native.status_list.size = usize::MAX;
        native.status_list.capacity = usize::MAX;
        assert!(matches!(decode_native(&native), Err(Error::RCL_RET_ERROR)));
        native.status_list.size = valid_size;
        native.status_list.capacity = valid_capacity;
        assert_eq!(
            decode_native(&native).unwrap(),
            super::super::decode_status_updates(&message).unwrap()
        );
    }

    #[test]
    #[ignore = "manual status-stage benchmark; use --release --ignored --nocapture"]
    fn benchmark_native_status_decode() {
        let count = std::env::var("R2R_STATUS_COUNT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1024);
        let iterations = std::env::var("R2R_BENCH_ITERATIONS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(10_000);
        assert!(iterations > 0);
        let native =
            NativeMsg::<action_msgs::msg::GoalStatusArray>::try_from(&fixture(count)).unwrap();
        let expected = decode_native(&native).unwrap();
        for mode in ["owned_rust_decode", "native_decode"] {
            let before = Instant::now();
            for _ in 0..iterations {
                let result = if mode == "owned_rust_decode" {
                    let message = action_msgs::msg::GoalStatusArray::from_native(
                        std::hint::black_box(&native),
                    );
                    super::super::decode_status_updates(&message).unwrap()
                } else {
                    decode_native(std::hint::black_box(&native)).unwrap()
                };
                std::hint::black_box(result);
            }
            let elapsed = before.elapsed().as_nanos();
            assert_eq!(decode_native(&native).unwrap(), expected);
            eprintln!(
                "{}",
                serde_json::json!({"mode":mode,"statuses":count,"iterations":iterations,"elapsed_ns":elapsed,"ns_per_op":elapsed as f64/iterations as f64})
            );
        }
    }
}
