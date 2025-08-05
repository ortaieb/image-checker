# Validating metadata and content - reject

Identify image but also check details about the image

## Assumptions
The request included image, stored in `$image_base_dir/image000003.png`

`$image_base_dir/image000003.png` is an image of road sign `Brentford`. The image was taken in (52.491079, -0.269590), on 2025-08-01T15:25:00Z+1.

## Input
{
  "processing-id": "000003",
  "image-path": "$image_base_dir/image000003.png",
  "analysis-request": {
    "content": "Pub sign `The Ale and Hops`",
    "location": "not more than 100m from coordinates (51.492191, -0.266108)",
    "datetime": "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1"
  }
}

## Outcome
{
  "processing-id": "000003",
  "results": {
    "resolution": "rejected"
    "resons": [
      "expected content was not found in the image",
      "Location of the image"
    ]
  }
}
