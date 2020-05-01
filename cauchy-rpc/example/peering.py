import grpc

import info_pb2
import info_pb2_grpc
import peering_pb2
import peering_pb2_grpc
from google.protobuf import empty_pb2
from time import time, sleep

empty = empty_pb2.Empty()

server_a_rpc = "127.0.0.1:2081"
server_b_rpc = "127.0.0.1:2080"
server_b = "127.0.0.1:1220"

with grpc.insecure_channel(server_a_rpc) as channel:
    peering_stub = peering_pb2_grpc.PeeringStub(channel)
    try:
        result = peering_stub.ConnectPeer(
            peering_pb2.ConnectRequest(address=server_b))
    except Exception as err:
        print("Failed to connect to", server_b)
        print(err)

    try:
        result = peering_stub.ListPeers(empty)
        print("Server A Peers:")
        print(result)
    except Exception as err:
        print("Failed list peers", err)
    

with grpc.insecure_channel(server_b_rpc) as channel:
    peering_stub = peering_pb2_grpc.PeeringStub(channel)

    try:
        result = peering_stub.ListPeers(empty)
        print("Server B Peers:")
        print(result)
    except Exception as err:
        print("Failed list peers", err)
    