import grpc

import transactions_pb2
import transactions_pb2_grpc

from google.protobuf import empty_pb2
from time import time, sleep
from threading import Thread

empty = empty_pb2.Empty()

with grpc.insecure_channel('127.0.0.1:2080') as channel:
    print("Info stub methods:")
    transactions_stub = transactions_pb2_grpc.TransactionsStub(channel)

    print("Loading transaction...")
    with open("contract_data.wasm", 'rb') as filehandle:

        binary = filehandle.read(1024*1024*1024)

        print("Broadcasting transaction...")
        transaction = transactions_pb2.Transaction(
            timestamp=123, binary=binary, aux_data=b"ABCD")
        threads = []
        transactions_stub.BroadcastTransaction(transaction)
