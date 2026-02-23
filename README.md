# Endpoint Data Management System (EDMS)
Endpoint data storage enabling versioning for accelerating developer collaboration

## Features
* **Webview** - Users can create a static website view for all the filtered set of endpoints
* **Repo-Ready** - Data can be generated in a way that's easy to push to a repo enabling versioning


## Core Idea 
Endpoints during the development phases keep changing continuously, and when several team members are working together, it gets tough to keep track of changes. This naturally introduces team coordination issues because a single key can break all the dependent microservices. An often overlooked piece of information is `endpoints` often have multiple pairs of requests and responses associated with it. This implies, all the allowed pairs per endpoint be stored, and documented, which often is not the case. 

This is where EDMS comes to the rescue. 

## Solution
* Endpoints are stored against all possible pairs of (requests, responses) as JSON files
* Auto generating table containing endpoint meta-data
* Create docs, and share in your repos enabling versioning of endpoint data
* Analytics to infer more about your endpoints




