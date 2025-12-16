# A Second Source
This feature will add an additional stream of data - Outdoor weather.  It will include both weather parameters as wells some outdoor air quality.  This pushes the platform further towards being a generic data platform.

## Openweathermap
We will be utilizing the free version of 2 of openweathermap API's.  We're using 2 because we need to collect all of the data attributes for the current weather in a specific location, as well as the air quality at the same location.  Since we are polling real time data, the API's update current data every 10 minutes, and we should plan on updating that frequently.  I've already put the OPENWEATHERMAP_API_KEY in a .env file.

Parameterize - Values used in the api calls should be parameters stored in etcd (like other config items).  This includes latitude, longitude, poll-interval, and even API urls, so they are not hardcoded.

Data Storage: Its important, when storing weather data that the timestamp is normalized to the same time format and timezone as our existing system.

### Current Weather
The guide for current weather api is found here: https://openweathermap.org/current  
We should capture all of the available information.  You must research how the API works to build the configuration and schema.

### Air Polution
This API gathers current particulate levels for the same locale.  Documentation found here: https://openweathermap.org/api/air-pollution#current

Interested in the current air quality at the current time.

## Build on Current Design
The current design of the neural data platform is found in docs/architecture.  Read the C4 architecture: docs/architecture/diagrams/neural-data-platform-c4.drawio.  

You are to EXTEND this platform, not rewrite it.

You'll also find new procedures for how to add a new stream and how to add a new data source in docs/procedures.

## Your mission
Should you choose to accept it:
1. Thoroughly investigate the API documentation to understand schema design and response expectations
2. FULLY understand the current architecture and procedures for how to make updates to the system.
3. Create full SPARC implementation documentation to create and integrate the functionality to add current weather and quality.

Store the SPARC documentation under product/features/air-005.  DO NOT IMPLEMENT THESE CHANGES.  I want to review the documentation first.  

