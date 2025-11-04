from django.http import HttpResponse


def socket(request):
    response = HttpResponse(status=307)
    response["Location"] = "http://localhost:6969/ws"
    return response
