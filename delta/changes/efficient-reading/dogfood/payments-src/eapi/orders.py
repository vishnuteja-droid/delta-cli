from src.papi.dispatch import dispatch_order

def handle_order(req):
    """Entry point: new order received from the web tier."""
    return dispatch_order(req)
