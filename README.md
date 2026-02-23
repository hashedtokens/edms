# Endpoint Data Management System (EDMS)
Endpoint data storage enabling versioning for accelerating developer collaboration

## Features
* **Web View** - Users can create a static website view for all the filtered set of endpoin
* **Repo Ready** - Data can be generated in a way that's easy to push to a repo enabling versioning
* **Test View** - Endpoints can be tested in real time with time out options
* **Filter/Merge Data** - Endpoints from multiple sources can be easily filtered into independent collections or merged into one collection


## Core Idea 
Endpoints during the development phases keep changing continuously, and when several team members are working together, it gets tough to keep track of changes. This naturally introduces team coordination issues because changing a single key can break all the dependent microservices. An often overlooked piece of information is `endpoints` often have multiple pairs of requests and responses associated with it. This implies, all the allowed pairs per endpoint be stored, and documented, which often is not the case. 

### Issue with Request/Response Pairs 
Be it `( dev, prod, test )`, developers write logic where, the key from the payload if different leads to a different response. 

Consider a simple endpoint `/my/test/`
```json
# request-1
{ k1: A,
  k2: v2
}

# response-1
{
  x: P
}

# request-2
{ k1: B,
  k2: v2
}

# response-2
{
  y: Q
}
```
Based on the example above, one can infer that, there's nothing really sacred about the requests and response, however when the data flows through multiple microservices where logic acts on these pairs per endpoint, it induces strain in the developer workflows. 

This is where EDMS comes to the rescue. 


## Solution
* Endpoints are stored against all valid pairs of (requests, responses) as JSON files
* Auto generating table containing endpoint meta-data
* Create docs, and share in your repos enabling versioning of endpoint data
* Analytics to infer more about your endpoints




