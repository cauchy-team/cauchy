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
        start_time = time()
        transaction = transactions_pb2.Transaction(
            timestamp=123, binary=binary, aux_data=b"ABCD")
        threads = []
        for _ in range(0, 1000):
            # thread = Thread(target = lambda: ())
            thread = Thread(target = lambda: transactions_stub.BroadcastTransaction(transaction))
            threads += [thread]
            thread.start()

        for thread in threads:
            thread.join()
        end_time = time()
        print("Done in", (end_time - start_time) / 1000, "seconds")
