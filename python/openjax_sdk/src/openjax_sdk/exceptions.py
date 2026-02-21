class OpenJaxProtocolError(RuntimeError):
    pass


class OpenJaxResponseError(RuntimeError):
    def __init__(self, code: str, message: str, retriable: bool, details: dict):
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message
        self.retriable = retriable
        self.details = details
