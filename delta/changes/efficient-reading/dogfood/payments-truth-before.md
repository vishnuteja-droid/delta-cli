# Payments

`eapi/orders.py` is the entry point for a new order. It calls
`papi/dispatch.py`, which posts synchronously to `otherservice`'s ledger API
and waits for the response before returning.
