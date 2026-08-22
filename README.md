# jitterbug



## Dependencies
- You must brew install yt-dlp


## Inspiration
This project was inpired a longstanding want for a simple surround-sound enabling tool. I'd initially tried a web-based version, [YouSync](https://github.com/skparab1/yousync), but realized that it was trying to sync from too high a level, and discrepancies arising from different browsers and other systems between the browser and the speaker made it difficult to achieve a jitter-free surround sound experience. This project aims to sit at a lower level, and has been more successful in syncing, in my experience. Note that the custom network in this project isn't really necessary, but it does transmit the minimum data necessary and is less complex than TCP (though I could have just used module, designing and implementing this protocol was fun).