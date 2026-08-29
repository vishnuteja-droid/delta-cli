# Findings — payments

## Entry points
- `handle_order` — `src/eapi/orders.py`

## Call chain
```
    handle_order ──► dispatch_order ──┄┄► ledger_client (otherservice)
                                            ▲
                                      ? retry count from LEDGER_RETRY_COUNT env var, default 3
```
`ledger_client` is otherservice's own client library (external import, no
local source) — recorded as an edge, not followed.

## Data touched
- otherservice's ledger, via `ledger_client.publish_async` (write, fire-and-forget)

## Unknowns
- Retry count is config-driven (`LEDGER_RETRY_COUNT`, default 3) — the
  actual runtime value depends on deployment config not visible here.

## Stale truth
- `delta/truth/payments.md` says dispatch "posts synchronously... and waits
  for the response before returning." The code (`src/papi/dispatch.py`)
  calls `ledger_client.publish_async` and returns immediately — fire and
  forget, not synchronous. Code wins; truth needs updating at the next
  archive.

---
read: 2 files (orders.py, dispatch.py) · truth: used (entry point + chain
partly answered, but contradicted on sync/async — see Stale truth) · bodies: 2
(both files were short enough that headers alone did not settle the
sync-vs-async question, which was the point of reading them)
