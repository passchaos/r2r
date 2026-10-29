use super::*;

#[test]
fn type_hash_parser_roundtrips_versions_and_rejects_malformed_identity() {
    for version in [0, 1, 2, 255] {
        let hash = TopicTypeHash {
            version,
            value: std::array::from_fn(|index| index as u8),
        };
        assert_eq!(hash.to_string().parse::<TopicTypeHash>().unwrap(), hash);
        assert_eq!(
            hash.to_string()
                .to_uppercase()
                .parse::<TopicTypeHash>()
                .unwrap(),
            hash
        );
    }
    for value in [
        "",
        "RIHS01",
        "RIHS01_aa",
        "RIHS01_",
        "RIHS00_🦀",
        "rihs01_0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        assert!(matches!(value.parse::<TopicTypeHash>(), Err(Error::InvalidTypeHash { .. })));
    }
    let mut invalid = TopicTypeHash {
        version: 1,
        value: [0; 32],
    }
    .to_string();
    invalid.replace_range(10..11, "g");
    assert!(invalid.parse::<TopicTypeHash>().is_err());
}

#[test]
fn generated_type_hashes_have_distribution_explicit_boundaries() {
    let message = message_type_hash::<crate::std_msgs::msg::UInt64>();
    let service = service_type_hash::<crate::example_interfaces::srv::AddTwoInts::Service>();
    let action = action_type_hash::<crate::example_interfaces::action::Fibonacci::Action>();
    #[cfg(r2r__ros__distro__humble)]
    {
        for result in [message, service, action] {
            assert!(matches!(result, Err(Error::RCL_RET_UNSUPPORTED)));
        }
    }
    #[cfg(not(r2r__ros__distro__humble))]
    {
        for result in [message, service, action] {
            let hash = result.unwrap();
            assert_eq!(hash.version, 1);
            assert_ne!(hash.value, [0; 32]);
        }
    }
}
