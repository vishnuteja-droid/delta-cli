import os
import ledger_client  # otherservice's client library

RETRY_COUNT = int(os.environ.get("LEDGER_RETRY_COUNT", "3"))

def dispatch_order(req):
    # Fire-and-forget: publishes to otherservice's queue and returns
    # immediately. Does NOT wait for a response.
    ledger_client.publish_async(req, retries=RETRY_COUNT)
    return {"status": "accepted"}
