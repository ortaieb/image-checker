# Validating metadata and content - reject

Identify image but also check details about the image

## Assumptions
The request did not included image

## Input
{
  "processing-id": "000004",
  "image": null,
  "analysis-request": {
    "content": "Pub sign `The Ale and Hops`",
    "location": "not more than 100m from coordinates (51.492191, -0.266108)",
    "datetime": "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1"
  }
}

## Outcome
{
  "processing-id": "000004",
  "results": {
    "resolution": "rejected"
    "resons": [
      "cannot locate image"
    ]
  }
}
