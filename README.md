# Endpoint Data Management System (EDMS)
_Standalone system for a team building microservices_

## Core Idea
During development stages `endpoints` may not change, but the associated QS pairs keep changing. The change is not just linked to the QS schemas which might be static or in rare cases well defined, but also related to the data bound to the keys contained in the QS. 

## Eliminate `Endpoint Chaos`
* Multiple servers talking to each other through QS pairs
* Simple change in an endpoint can lead to a systemic mess
* Team members involved in managing microservices experience coordination delays
         - endpoints not versioned
         - endpoints not mapped against QS pairs
         - no summary of endpoint inventory

So, EDMS helps to reduce the chaos so that development progresses without hassle regardless of the team size, without having to compromise speed or efficiency.  

## Features
* **Web View** - Users can create a static website view for all the filtered set of endpoints
* **Repo Ready** - Data can be generated in a way that's easy to push to a repo enabling versioning
* **Test View** - Endpoints can be tested in real time with time out options
* **Filter / Merge Data** - Endpoints from multiple sources can be easily filtered into independent collections or merged into one collection

### Abbreviations
* `E` : **Endpoint** 
* `Q` : **Request**
* `S` : **Response**
