# Jitterbug
A network protocol and terminal application for orchestrating synchronized audio playback across devices.

# Purpose
Making audio play at the exact same time on multiple devices is difficult. Differences in network speed and operating system make perfect synchronization challenging, often leading to chaotic echos and discernable lag.
This project has each client independently load audio content over third party,

# How to use

## Dependencies
- The following dependencies are necessary
```bash
brew install ffmpeg
brew install yt-dlp
```

## Server commands
- Spin up a server
```bash
cargo run --bin server
```

- Commands:
    - `load <youtube URL>`: Downloads an audio file for the


# Inspiration
This project was inpired a longstanding want for a simple music syncing tool. I'd initially tried a web-based version, [YouSync](https://github.com/skparab1/yousync), but realized that it was trying to sync from too high a level, and discrepancies arising from different browsers and other systems between the browser and the speaker made it difficult to achieve a jitter-free surround sound experience. This project aims to sit at a lower level, and has been more successful in syncing, in my experience. Note that the custom network in this project isn't really necessary, but it does transmit the minimum data necessary and is less complex than TCP (though I could have just used module, designing and implementing this protocol was fun).

# How it works
## Playback
The basic flow for playback syncing is as follows: 
- The server sends a `load <URL>` command. This tells the client: "independently download the audio content from the given URL, and report when done". The client reports when the file is downloaded, and adds it to its queue.
- The server sends a `swap` command. This tells the client: "swap in the file at the top of the queue and buffer it to 00:00. Be ready for my play signal".
- The server sends a `play` command. This signal comes with a specific playback timestamp (like 01:11) and a unix timestamp. This tells the client: "wait until `unix timestamp`, then play the current audio at `timestamp`".
- Instructions like volume, and pause are actioned immediately.

## Protocol
### Frame
- The frame was meant to be minimal. It looks like:
    - magic bytes: 4 bytes      [raw]
    - nonce: 12 bytes           [raw]
    - packet type: 1 byte       [encrypted]
    - sequence number: 4 bytes  [encrypted]
    - payload: up to 235 bytes  [encrypted]
- The encrypted portion is encrypted using AES-GCM-128, with a pre-shared-key (see below) the attached nonce.

### Handshake
- PSK share: The server generates a common pre-shared-key (PSK), which will be shared with all clients out of band (by text).
- SYN: The client generates a nonce, encrypts it with the the PSK (AES-GCM-128), and sends it to the server.
- The server decrypts the payload, obtaining the nonce. Since this is AEAD, if the client does not have the PSK, this will fail.
- ACK: The server re-encrypts the nonce with the PSK, and sends it back to the client. This reply looks different because the AES-GCM-128 nonce is difference.
- Verify: The client verifies that returned nonce matches, and confirms the connection

## Terminal Application
- Made using ratatui
