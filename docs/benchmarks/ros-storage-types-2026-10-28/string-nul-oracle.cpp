#include <rclcpp/serialization.hpp>
#include <rclcpp/serialized_message.hpp>
#include <rcl_interfaces/msg/parameter_value.hpp>
#include <iostream>
int main() {
 rcl_interfaces::msg::ParameterValue source; source.type=9;
 source.string_array_value={std::string("head\0tail",9)};
 rclcpp::Serialization<rcl_interfaces::msg::ParameterValue> serializer;
 rclcpp::SerializedMessage bytes; serializer.serialize_message(&source,&bytes);
 rcl_interfaces::msg::ParameterValue observed; serializer.deserialize_message(&bytes,&observed);
 std::cout << "{\"cdr_matches\":" << (source==observed ? "true":"false")
 << ",\"wire_bytes\":" << bytes.size() << ",\"input_bytes\":9,\"observed_bytes\":"
 << observed.string_array_value.at(0).size() << "}\n";
}
