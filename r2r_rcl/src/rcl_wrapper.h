// Workaround for _Check_return_ annotation
#ifdef _WIN32
#ifndef _Check_return_
#define _Check_return_
#endif
#endif

// ros client library
#include <rcl/rcl.h>

// query the network
#include <rcl/graph.h>
#if __has_include(<rcl/network_flow_endpoints.h>)
#include <rcl/network_flow_endpoints.h>
#endif

// logging
#include <rcl/logging.h>

// managed/lifecycle nodes
#if __has_include(<rcl_lifecycle/rcl_lifecycle.h>)
#include <rcl_lifecycle/rcl_lifecycle.h>
#endif

// errors
#include <rcutils/error_handling.h>

// low level msg type handling
#include <rosidl_runtime_c/string.h>
#include <rosidl_runtime_c/string_functions.h>

#include <rosidl_runtime_c/u16string.h>
#include <rosidl_runtime_c/u16string_functions.h>

#include <rosidl_runtime_c/primitives_sequence.h>
#include <rosidl_runtime_c/primitives_sequence_functions.h>

#include <rosidl_runtime_c/message_type_support_struct.h>
#include <rosidl_runtime_c/service_type_support_struct.h>
#include <rosidl_runtime_c/action_type_support_struct.h>
#include <rosidl_typesupport_introspection_c/message_introspection.h>
#include <rosidl_typesupport_introspection_c/field_types.h>

// parameters
#include <rcl_yaml_param_parser/parser.h>
