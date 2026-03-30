# Endpoint Data Management System (EDMS)
_Standalone system for a team building microservices_

## Core Idea
During development stages `endpoints` may not change, but the associated QP pairs keep changing. The change is not just linked to the QP schemas which might be static or in rare cases well defined, but also related to the data bound to the keys contained in the QP. 

## Eliminate `EQP Chaos`
* _Communication breaks_ when multiple servers talking to each other with evolving QP pairs per E
* _Systemic mess_ when a simple change in either of `E, Q or P` happens
* _Coordination delays & technical debt_ happens when several team members are modifying microservice data
  - endpoints not versioned
  - endpoints not mapped against all the linked QP pairs per E
  - no summary of endpoint inventory
  - no inbuilt tool for doc generation of EQP pairs

So, EDMS helps to reduce the chaos so that development progresses without hassle regardless of the team size, without having to compromise speed or efficiency.  

## Features
* **Web View** - Users can create a static website view for all the filtered set of endpoints
* **Repo Ready** - Data can be generated in a way that's easy to push to a repo enabling versioning
* **Test View** - Endpoints can be tested in real time with time out options
* **Filter / Merge Data** - Endpoints from multiple sources can be easily filtered into independent collections or merged into one collection

### Symbols 
* `E` : **Endpoint** 
* `Q` : **Request**
* `P` : **Response**
