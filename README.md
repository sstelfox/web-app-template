# Web App Template
This is a general playground I use for integrating different pieces of Rust crates into an actual web-app. A lot of this is experimental and left broken as I figured out enough of the pattern for use elsewhere. I may license the code specifically eventually but have intentionally left it out for now.

## Development Environment

Compiling the CSS:

```
npm exec tailwindcss -- -i styles/app.css -o dist/css/app.css
```

That can have a `--watch` flag added to it for live changes.

Rust is straight-forward, cargo run though some environment variables in the `.env` file need to be setup first.
