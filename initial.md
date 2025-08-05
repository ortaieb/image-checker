## Feature:
Image-Checker is an advanced AI Agent confirming an image meet list of expectations. It will receive an image file and will validate the following:
- The image meet expected summary
- Image taken close enough to specific location
- Image taken on a specified time window

The image will receive a list of expectations, send it to the LLM with the image file and receive back resolution (accept/reject) and a list the reasoning behind decision when rejected.

### Design for parallel use
The `Image-Checker` expected to handle multiple requests in parallel
- Plan each processing to operate asynchronously, here's an example:
  - accept request and assign processing-id
  - validate input
  - send response back with processing-id and status of `accepted` (or `invalid`)
  - place the processing data in a queue (size in env-var)
  - client will have an endpoint to check status of request
  - when completed the user can send request to extract data
- Agents working with a third party models might require throttling to control budget and resources. Add throttling (request in minutes)
- If queue is full request will be rejected with retryable status allowing the client to resend the request

### Cancellations
The agent will keep tracks over time and will cancel process after agreed duration set on startup


## EXAMPLES & DOCUMENTATION:

### Examples
Check examples under directory `examples/` it will provide with the desired behaviour of the agent.

### Documentation:
- For processing metadata from an image use [EXIT](https://docs.rs/kamadak-exif/latest/exif/)
- Find more details about prompting with LLava

## OTHER CONSIDERATIONS:

- The agent will be written in Rust
- Agent's attributes (e.g. url and name of the model, request timeout etc) should be planned as environment variables allowing changes without changing the code itself
- As testing model the agent will use LLaVa. Later, based on performance, there might be a change to managed model, make sure both method will work
- Use a full uri to descirbe location of image. Have the image_base_dir value as env var. Allowing the agent to use local storage or cloud managed solution (gcs/s2/blob). For the later, if required, consider additional security details be added (secret/auth key of any kind).
- Looks like LLaVa can only provide image analysis, the checks of the metadata should be done by the agent itself.
