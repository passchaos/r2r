use super::{WrappedNativeMsg, WrappedTypesupport};
use crate::std_msgs::msg::{Int32MultiArray, MultiArrayDimension, MultiArrayLayout};

fn array(dimensions: usize, elements: usize) -> Int32MultiArray {
    Int32MultiArray {
        layout: MultiArrayLayout {
            dim: (0..dimensions)
                .map(|index| MultiArrayDimension {
                    label: format!("axis_{index}"),
                    size: elements as u32,
                    stride: elements as u32,
                })
                .collect(),
            data_offset: dimensions as u32,
        },
        data: (0..elements).map(|index| index as i32 - 11).collect(),
    }
}

#[test]
fn native_sequences_reuse_initialized_capacity_across_empty_and_shrinking_updates() {
    let mut native = WrappedNativeMsg::<Int32MultiArray>::try_from(&array(3, 8)).unwrap();
    let data = native.data.data;
    let dimensions = native.layout.dim.data;
    for (dimension_count, element_count) in [(1, 2), (0, 0), (2, 4), (3, 8)] {
        let message = array(dimension_count, element_count);
        message.try_copy_to_native(&mut native).unwrap();
        assert_eq!(native.data.data, data);
        assert_eq!(native.data.capacity, 8);
        assert_eq!(native.layout.dim.data, dimensions);
        assert_eq!(native.layout.dim.capacity, 3);
        assert_eq!(Int32MultiArray::from_native(&native), message);
        assert_eq!(native.data.size, element_count);
        assert_eq!(native.layout.dim.size, dimension_count);
    }
    let grown = array(5, 16);
    grown.try_copy_to_native(&mut native).unwrap();
    assert_eq!(Int32MultiArray::from_native(&native), grown);
    let data = native.data.data;
    let dimensions = native.layout.dim.data;
    let smaller = array(1, 1);
    smaller.try_copy_to_native(&mut native).unwrap();
    assert_eq!(native.data.data, data);
    assert_eq!(native.layout.dim.data, dimensions);
    assert_eq!(Int32MultiArray::from_native(&native), smaller);
}

#[test]
fn nested_trajectory_sequences_preserve_hidden_initialized_elements() {
    use crate::trajectory_msgs::msg::{JointTrajectory, JointTrajectoryPoint};

    let points = |count, width| {
        (0..count)
            .map(|point| JointTrajectoryPoint {
                positions: (0..width)
                    .map(|value| (point * 10 + value) as f64)
                    .collect(),
                velocities: vec![1.0; width],
                accelerations: vec![2.0; width],
                effort: vec![3.0; width],
                ..Default::default()
            })
            .collect()
    };
    let message = JointTrajectory {
        points: points(3, 4),
        ..Default::default()
    };
    let mut native = WrappedNativeMsg::<JointTrajectory>::try_from(&message).unwrap();
    let outer = native.points.data;
    let first_positions = unsafe { (*outer).positions.data };
    for (count, width) in [(1, 1), (0, 0), (2, 2), (3, 4)] {
        let message = JointTrajectory {
            points: points(count, width),
            ..Default::default()
        };
        message.try_copy_to_native(&mut native).unwrap();
        assert_eq!(native.points.data, outer);
        assert_eq!(native.points.capacity, 3);
        assert_eq!(unsafe { (*outer).positions.data }, first_positions);
        assert_eq!(JointTrajectory::from_native(&native), message);
    }
}
