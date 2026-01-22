import json

from django.http import JsonResponse
from django.shortcuts import render
from django.views.decorators.csrf import csrf_exempt
from django.views.decorators.http import require_POST
from webrockets import broadcast


def index(request):
    """Render the chat interface."""
    return render(request, "index.html")


@csrf_exempt
@require_POST
def broadcast_message(request):
    """
    API endpoint to broadcast a message from Django.

    This demonstrates how to send messages from Django to WebSocket clients
    using the broadcast() function and Redis broker.
    """
    try:
        data = json.loads(request.body)
        room = data.get("room", "general")
        message = data.get("message", "")
        sender = data.get("sender", "Server")

        if not message:
            return JsonResponse({"error": "Message is required"}, status=400)

        # Broadcast to all clients in the room via Redis
        broadcast(
            [room],
            json.dumps(
                {
                    "type": "message",
                    "sender": sender,
                    "message": message,
                    "from_api": True,
                }
            ),
        )

        return JsonResponse({"status": "sent", "room": room})

    except json.JSONDecodeError:
        return JsonResponse({"error": "Invalid JSON"}, status=400)
