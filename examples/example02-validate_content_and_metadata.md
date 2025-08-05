# Validating metadata and content - accept

Identify image but also check details about the image

## Assumptions
The request included image, stored in `$image_base_dir/image000002.png`

`$image_base_dir/image000002.png` is an image of `The Ale and Hops` pub sign. The image was taken in (51.491079, -0.269590), on 2025-08-01T15:25:00Z+1.

## Input
{
  "processing-id": "000002",
  "image-path": "$image_base_dir/image000002.png",
  "analysis-request": {
    "content": "Pub sign `The Ale and Hops`",
    "location": "not more than 100m from coordinates (51.492191, -0.266108)",
    "datetime": "image was taken not more than 10 minutes after 2025-08-01T15:23:00Z+1"
  }
}

## Outcome
{
  "processing-id": "000002",
  "results": {
    "resolution": "accepted"
  }
}
