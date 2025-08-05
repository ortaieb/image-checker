# Just Summary check

Check only content of the image and ignore any metadata

## Assumptions
The request included image, stored in `$image_base_dir/image000001.png`

`$image_base_dir/image000001.png` is an image of sky, with three pegions sitting on a wire at the bottom right of the frame.

## Input
{
  "processing-id": "000001",
  "image-path": "$image_base_dir/image000001.png",
  "analysis-request": {
    "image-path": "$image_base_dir/image000001.png",
    "content": "Three birds on a wire"
  }
}

## Outcome
{
  "processing-id": "000001",
  "results": {
    "resolution": "accepted"
  }
}
