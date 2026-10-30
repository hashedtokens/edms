# Packages
While testing endpoints, you may want to move the data into EDMS directly and receive a response for other purposes. `EDMS library` is essentially a wrapper around widely used REST API libraries to make sure just hit the endpoint once but achieve both the goals. 

EDMS acts like your proxy server for storing the EQP data and retrieving the response data. 

## Purpose
* Test your endpoint
* Store your EQP Data into EDMS
* Get the response and use it in your application logic

`Note:` Multiple languages are supported

## How to use it
Visit the Wiki for more instructions 
https://github.com/hashedtokens/edms/wiki/Packages---Imports

The rough idea is given below, syntax varies by language
```	
import hashedtokens as ht
response = ht.request(URL, payload, CRUDType)

# response_object that can be directly consumed and
# EDMS output if successful or failure
response = (response_obj, ht_response)

// do something with the output
application_logic(response.response_obj)
```



