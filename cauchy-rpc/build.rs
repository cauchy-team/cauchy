fn main() {
    tonic_build::compile_protos("proto/peering.proto").expect("failed to compile protobuf");
    tonic_build::compile_protos("proto/info.proto").expect("failed to compile protobuf");
}