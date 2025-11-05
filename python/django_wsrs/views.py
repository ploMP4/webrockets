from typing import ClassVar
from django.http import HttpRequest, HttpResponse


class SocketView:
    group: ClassVar[str]

    def connect(self): ...
    def receive(self, data): ...
    def disconnect(self): ...

    @classmethod
    def as_view(cls):
        def view():
            response = HttpResponse(status=307)
            response["Location"] = "http://localhost:6969/ws"
            return response

        return view


def socket(_: HttpRequest) -> HttpResponse:
    response = HttpResponse(status=307)
    response["Location"] = "http://localhost:6969/ws"
    return response
