def helper(message):
    print(message)
    return message


def _unreachable_orphan():
    """Private function never called anywhere — will be detected as dead code."""
    pass
