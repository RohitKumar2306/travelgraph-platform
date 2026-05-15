import os

import httpx
from fastapi import FastAPI
from pydantic import BaseModel


class ReviewRequest(BaseModel):
    oldSchema: str
    newSchema: str
    serviceName: str
    ownerTeam: str


class ReviewResponse(BaseModel):
    markdown: str


app = FastAPI(title="TravelGraph AI Schema Assistant")


@app.post("/review", response_model=ReviewResponse)
async def review_schema(req: ReviewRequest) -> ReviewResponse:
    api_key = os.getenv("ANTHROPIC_API_KEY")
    if not api_key:
        return ReviewResponse(markdown="AI review unavailable")

    prompt = f"""
You are an advisory GraphQL schema reviewer for TravelGraph Platform.
This review is never blocking. Return concise markdown with these sections:

## Change Summary
## Breaking Change Explanation
## Migration Path Suggestions
## Naming And Documentation Issues
## Deprecation Recommendations

Service: {req.serviceName}
Owner team: {req.ownerTeam}

Old schema:
```graphql
{req.oldSchema}
```

New schema:
```graphql
{req.newSchema}
```
"""

    try:
        async with httpx.AsyncClient(timeout=30) as client:
            response = await client.post(
                "https://api.anthropic.com/v1/messages",
                headers={
                    "x-api-key": api_key,
                    "anthropic-version": "2023-06-01",
                    "content-type": "application/json",
                },
                json={
                    "model": os.getenv("ANTHROPIC_MODEL", "claude-3-5-sonnet-latest"),
                    "max_tokens": 1200,
                    "messages": [{"role": "user", "content": prompt}],
                },
            )
            response.raise_for_status()
            data = response.json()
            text = "".join(
                block.get("text", "")
                for block in data.get("content", [])
                if block.get("type") == "text"
            ).strip()
            return ReviewResponse(markdown=text or "AI review unavailable")
    except Exception:
        return ReviewResponse(markdown="AI review unavailable")


@app.get("/health")
async def health() -> dict[str, str]:
    return {"status": "ok"}
