import json

print(json.dumps({
    "schema": "project.ops.status/v1",
    "project": "sprocket-fixture",
    "generated_at": "2026-08-25T12:00:00Z",
    "manifest": {
        "schema": "project.concerns/v1",
        "path": ".ops/concerns.toml",
    },
    "producer": {"id": "sprocket-fixture.status", "session_id": "fifth-fixture"},
    "authority": {"kind": "producer-testimony-only"},
    "concerns": [
        {
            "id": "sprocket.queue.bounded",
            "question": "sprocket.question.queue-bounded/v1",
            "profile": "sprocket.profile.queue-bounded-17/v1",
            "required": True,
            "description": "Is the supplied queue depth at most the governed bound of 17?",
            "observation": {
                "observation_present": True,
                "local_state": "FROBNICATED",
                "domain_state": "SPROCKET_PAUSED",
                "observed_at": "2026-08-25T12:00:00Z",
                "valid_for_seconds": 300,
                "reason": "opaque state is deliberately irrelevant to the governed predicate",
                "facts": {"queue": {"depth": 12}},
            },
        }
    ],
}))
