# Endpoint Data Management System (EDMS)
_Standalone system for a team building microservices efficiently_

## Features
* **Web View** - Users can create a static website view for all the filtered set of endpoints
* **Repo Ready** - Data can be generated in a way that's easy to push to a repo enabling versioning
* **Test View** - Endpoints can be tested in real time with time out options
* **Filter / Merge Data** - Endpoints from multiple sources can be easily filtered into independent collections or merged into one collection


## Core Idea 
Endpoints during the development phases keep changing continuously, and when several team members are working together, it gets tough to keep track of changes. This naturally introduces team coordination issues because changing a single key can break all the dependent microservices. 

Designing endpoints well in advance is generally not possible, and prevents continuous iterations, as systems keep evolving over time. Accumulating complexity can be refactored, but not necessarily completely eliminated with a strict design in place. Flexibility to modify endpoint data accelerates the overall development process. 

An often overlooked piece of information is `endpoints` often have multiple pairs of requests and responses associated with it. This implies, all the allowed pairs per endpoint be stored, and documented, which often is not the case. 


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
Based on the example above, one can infer that, there's nothing really sacred about the requests and response, however when the data flows through multiple microservices where logic acts on these pairs per endpoint, it induces strain in the developer workflows. Look at how multiple devs are modifying the requests / responses for the same endpoints consumed by different servers. 

**Individual Developer Logic in a Team**
```
dev-A:server-1(dev): logic(/my/test) 
dev-B:server-3(prod): logic(/my/test)
dev-C:server-2(dev): logic(/my/test)
dev-D:server-1(test): logic(/my/test)
```

This is where EDMS comes to the rescue. It's designed to streamline the following
* _team coordination_ where endpoints keep rapidly evolving, but devs stays upto date
* _request / response sets_ bound to a specific endpoint, whenever a new variant shows up
* _versioning endpoint data_, where data is stored in a repo and devs can quickly cross check or make changes with the benefits of version control


## Solution
* Endpoints are stored against all valid pairs of (requests, responses) as JSON files
* Auto generating table containing endpoint meta-data
* Create docs, and share in your repos enabling versioning of endpoint data
* Analytics to infer more about your endpoints




