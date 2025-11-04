from django.http import HttpRequest, HttpResponse


def socket(_: HttpRequest) -> HttpResponse:
    response = HttpResponse(status=307)
    response["Location"] = "http://localhost:6969/ws"
    return response
