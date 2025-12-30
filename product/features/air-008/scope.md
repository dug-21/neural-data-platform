# Home Events
## Background
We now have active monitoring of a variety of parameters related to air quality monitoring both indoor and outdoor streams.  This shows the current and forecasted (outdoor) values.  Another critical metric that impacts indoor air quality is whether the windows are open in the house.  This is more difficult to determine, and is a manual change/event.

An additional decision was that I am going to leverage Home Assistant to help collect home sensor information.  This will provide access to the events/state related to windows in my house.  This attribute has a direct effect on inside temperature and humidity.

## Scope
The goal of this feature, is to 1. Determine the data architecture that will be most valuable to including this in our data for analytics. (my long term goal is to develop neural prediction as to when to open/close the windows for optimal air quality).  For now, I want to determine how to structure the backend data.  Next will be to create an interface so that I can easily modify the settings.(design of the interface is excluded from scope, but creating the specification how it writes the data is included).  The key question is whether to manage state change, or event based with context (state would need to be derived).  As we are building a generic platform, this has larger implications than just the air quality related stream.  2. Now consider the collection of data and analytics for a completely different category of data (such as log streams from systems).  This is not imminent, however I want to take this into account in the data architecture decisions.

## Research
Research broad data platform strategies for time series data platform.  Include research of Home Assistant Data approach: https://data.home-assistant.io. I'm not planning on highly utilizing the HA data science portal itself, per say, however, they may have strategies and structures that could be helpful/useful.  If you see value in this application of using their portal, say so.  Ensure your research is broader, though than just the home assistant view.

## Storage
Store all of the research, analysis and recommendations in product/research/dp-analysis.
