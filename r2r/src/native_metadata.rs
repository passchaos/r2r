//! Safe boundaries for partially populated native service metadata.

use std::{mem::MaybeUninit, ptr};

use r2r_rcl::{rmw_request_id_t, rmw_service_info_t};

/// Initialize optional metadata before handing its storage to RCL/RMW.
/// Some middleware leaves response GUIDs or unsupported timestamps untouched.
pub(crate) fn service_info() -> rmw_service_info_t {
    rmw_service_info_t {
        source_timestamp: 0,
        received_timestamp: 0,
        request_id: rmw_request_id_t {
            writer_guid: [0; 16],
            sequence_number: 0,
        },
    }
}

/// Read the mandatory response sequence without forming a request-id value
/// containing an optional, potentially uninitialized writer GUID.
///
/// # Safety
/// The successful native response take must have initialized `sequence_number`.
pub(crate) unsafe fn response_sequence_number(header: &MaybeUninit<rmw_request_id_t>) -> i64 {
    ptr::addr_of!((*header.as_ptr()).sequence_number).read()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_metadata_retains_zero_for_fields_the_middleware_does_not_write() {
        let mut native = service_info();
        native.request_id.sequence_number = 42;
        let response = crate::ServiceResponseInfo::from(&native);
        let request = crate::ServiceRequestInfo::from(&native);
        assert_eq!(response.sequence_number, 42);
        assert_eq!(request.sequence_number, 42);
        assert_eq!(response.writer_guid, [0; 16]);
        assert_eq!(request.writer_guid, [0; 16]);
        assert_eq!(response.source_timestamp, 0);
        assert_eq!(response.received_timestamp, 0);
        assert_eq!(request.source_timestamp, 0);
        assert_eq!(request.received_timestamp, 0);
    }

    #[test]
    fn action_response_reads_only_the_initialized_sequence_field() {
        let mut header = MaybeUninit::<rmw_request_id_t>::uninit();
        unsafe {
            ptr::addr_of_mut!((*header.as_mut_ptr()).sequence_number).write(-42);
            assert_eq!(response_sequence_number(&header), -42);
        }
    }
}
