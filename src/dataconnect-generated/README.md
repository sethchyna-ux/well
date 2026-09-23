# Generated TypeScript README
This README will guide you through the process of using the generated JavaScript SDK package for the connector `example`. It will also provide examples on how to use your generated SDK to call your Data Connect queries and mutations.

***NOTE:** This README is generated alongside the generated SDK. If you make changes to this file, they will be overwritten when the SDK is regenerated.*

# Table of Contents
- [**Overview**](#generated-javascript-readme)
- [**Accessing the connector**](#accessing-the-connector)
  - [*Connecting to the local Emulator*](#connecting-to-the-local-emulator)
- [**Queries**](#queries)
  - [*GetCurrentUser*](#getcurrentuser)
  - [*ListAllUsers*](#listallusers)
  - [*GetProfile*](#getprofile)
  - [*ListMyProfiles*](#listmyprofiles)
  - [*GetShellConfig*](#getshellconfig)
  - [*ListMyShellConfigs*](#listmyshellconfigs)
  - [*GetCommandHistory*](#getcommandhistory)
  - [*ListMyCommandHistory*](#listmycommandhistory)
  - [*GetTerminalTheme*](#getterminaltheme)
  - [*ListMyTerminalThemes*](#listmyterminalthemes)
  - [*GetDeviceSync*](#getdevicesync)
  - [*ListMyDevices*](#listmydevices)
- [**Mutations**](#mutations)
  - [*CreateUser*](#createuser)
  - [*UpdateUser*](#updateuser)
  - [*DeleteUser*](#deleteuser)
  - [*CreateProfile*](#createprofile)
  - [*UpdateProfile*](#updateprofile)
  - [*DeleteProfile*](#deleteprofile)
  - [*CreateShellConfig*](#createshellconfig)
  - [*UpdateShellConfig*](#updateshellconfig)
  - [*DeleteShellConfig*](#deleteshellconfig)
  - [*AddCommandHistory*](#addcommandhistory)
  - [*DeleteCommandHistory*](#deletecommandhistory)
  - [*CreateTerminalTheme*](#createterminaltheme)
  - [*UpdateTerminalTheme*](#updateterminaltheme)
  - [*DeleteTerminalTheme*](#deleteterminaltheme)
  - [*SyncDevice*](#syncdevice)
  - [*UpdateDeviceSync*](#updatedevicesync)
  - [*DeleteDeviceSync*](#deletedevicesync)

# Accessing the connector
A connector is a collection of Queries and Mutations. One SDK is generated for each connector - this SDK is generated for the connector `example`. You can find more information about connectors in the [Data Connect documentation](https://firebase.google.com/docs/data-connect#how-does).

You can use this generated SDK by importing from the package `@dataconnect/generated` as shown below. Both CommonJS and ESM imports are supported.

You can also follow the instructions from the [Data Connect documentation](https://firebase.google.com/docs/data-connect/web-sdk#set-client).

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig } from '@dataconnect/generated';

const dataConnect = getDataConnect(connectorConfig);
```

## Connecting to the local Emulator
By default, the connector will connect to the production service.

To connect to the emulator, you can use the following code.
You can also follow the emulator instructions from the [Data Connect documentation](https://firebase.google.com/docs/data-connect/web-sdk#instrument-clients).

```typescript
import { connectDataConnectEmulator, getDataConnect } from 'firebase/data-connect';
import { connectorConfig } from '@dataconnect/generated';

const dataConnect = getDataConnect(connectorConfig);
connectDataConnectEmulator(dataConnect, 'localhost', 9399);
```

After it's initialized, you can call your Data Connect [queries](#queries) and [mutations](#mutations) from your generated SDK.

# Queries

There are two ways to execute a Data Connect Query using the generated Web SDK:
- Using a Query Reference function, which returns a `QueryRef`
  - The `QueryRef` can be used as an argument to `executeQuery()`, which will execute the Query and return a `QueryPromise`
- Using an action shortcut function, which returns a `QueryPromise`
  - Calling the action shortcut function will execute the Query and return a `QueryPromise`

The following is true for both the action shortcut function and the `QueryRef` function:
- The `QueryPromise` returned will resolve to the result of the Query once it has finished executing
- If the Query accepts arguments, both the action shortcut function and the `QueryRef` function accept a single argument: an object that contains all the required variables (and the optional variables) for the Query
- Both functions can be called with or without passing in a `DataConnect` instance as an argument. If no `DataConnect` argument is passed in, then the generated SDK will call `getDataConnect(connectorConfig)` behind the scenes for you.

Below are examples of how to use the `example` connector's generated functions to execute each query. You can also follow the examples from the [Data Connect documentation](https://firebase.google.com/docs/data-connect/web-sdk#using-queries).

## GetCurrentUser
You can execute the `GetCurrentUser` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
getCurrentUser(options?: ExecuteQueryOptions): QueryPromise<GetCurrentUserData, undefined>;

interface GetCurrentUserRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<GetCurrentUserData, undefined>;
}
export const getCurrentUserRef: GetCurrentUserRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
getCurrentUser(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<GetCurrentUserData, undefined>;

interface GetCurrentUserRef {
  ...
  (dc: DataConnect): QueryRef<GetCurrentUserData, undefined>;
}
export const getCurrentUserRef: GetCurrentUserRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the getCurrentUserRef:
```typescript
const name = getCurrentUserRef.operationName;
console.log(name);
```

### Variables
The `GetCurrentUser` query has no variables.
### Return Type
Recall that executing the `GetCurrentUser` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `GetCurrentUserData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface GetCurrentUserData {
  user?: {
    username: string;
    email: string;
  };
}
```
### Using `GetCurrentUser`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, getCurrentUser } from '@dataconnect/generated';


// Call the `getCurrentUser()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await getCurrentUser();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await getCurrentUser(dataConnect);

console.log(data.user);

// Or, you can use the `Promise` API.
getCurrentUser().then((response) => {
  const data = response.data;
  console.log(data.user);
});
```

### Using `GetCurrentUser`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, getCurrentUserRef } from '@dataconnect/generated';


// Call the `getCurrentUserRef()` function to get a reference to the query.
const ref = getCurrentUserRef();

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = getCurrentUserRef(dataConnect);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.user);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.user);
});
```

## ListAllUsers
You can execute the `ListAllUsers` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
listAllUsers(options?: ExecuteQueryOptions): QueryPromise<ListAllUsersData, undefined>;

interface ListAllUsersRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListAllUsersData, undefined>;
}
export const listAllUsersRef: ListAllUsersRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
listAllUsers(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListAllUsersData, undefined>;

interface ListAllUsersRef {
  ...
  (dc: DataConnect): QueryRef<ListAllUsersData, undefined>;
}
export const listAllUsersRef: ListAllUsersRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the listAllUsersRef:
```typescript
const name = listAllUsersRef.operationName;
console.log(name);
```

### Variables
The `ListAllUsers` query has no variables.
### Return Type
Recall that executing the `ListAllUsers` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `ListAllUsersData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface ListAllUsersData {
  users: ({
    username: string;
    avatarUrl?: string | null;
  })[];
}
```
### Using `ListAllUsers`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, listAllUsers } from '@dataconnect/generated';


// Call the `listAllUsers()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await listAllUsers();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await listAllUsers(dataConnect);

console.log(data.users);

// Or, you can use the `Promise` API.
listAllUsers().then((response) => {
  const data = response.data;
  console.log(data.users);
});
```

### Using `ListAllUsers`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, listAllUsersRef } from '@dataconnect/generated';


// Call the `listAllUsersRef()` function to get a reference to the query.
const ref = listAllUsersRef();

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = listAllUsersRef(dataConnect);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.users);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.users);
});
```

## GetProfile
You can execute the `GetProfile` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
getProfile(vars: GetProfileVariables, options?: ExecuteQueryOptions): QueryPromise<GetProfileData, GetProfileVariables>;

interface GetProfileRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetProfileVariables): QueryRef<GetProfileData, GetProfileVariables>;
}
export const getProfileRef: GetProfileRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
getProfile(dc: DataConnect, vars: GetProfileVariables, options?: ExecuteQueryOptions): QueryPromise<GetProfileData, GetProfileVariables>;

interface GetProfileRef {
  ...
  (dc: DataConnect, vars: GetProfileVariables): QueryRef<GetProfileData, GetProfileVariables>;
}
export const getProfileRef: GetProfileRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the getProfileRef:
```typescript
const name = getProfileRef.operationName;
console.log(name);
```

### Variables
The `GetProfile` query requires an argument of type `GetProfileVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface GetProfileVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `GetProfile` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `GetProfileData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface GetProfileData {
  profile?: {
    name: string;
    metadata?: string | null;
  };
}
```
### Using `GetProfile`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, getProfile, GetProfileVariables } from '@dataconnect/generated';

// The `GetProfile` query requires an argument of type `GetProfileVariables`:
const getProfileVars: GetProfileVariables = {
  id: ..., 
};

// Call the `getProfile()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await getProfile(getProfileVars);
// Variables can be defined inline as well.
const { data } = await getProfile({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await getProfile(dataConnect, getProfileVars);

console.log(data.profile);

// Or, you can use the `Promise` API.
getProfile(getProfileVars).then((response) => {
  const data = response.data;
  console.log(data.profile);
});
```

### Using `GetProfile`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, getProfileRef, GetProfileVariables } from '@dataconnect/generated';

// The `GetProfile` query requires an argument of type `GetProfileVariables`:
const getProfileVars: GetProfileVariables = {
  id: ..., 
};

// Call the `getProfileRef()` function to get a reference to the query.
const ref = getProfileRef(getProfileVars);
// Variables can be defined inline as well.
const ref = getProfileRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = getProfileRef(dataConnect, getProfileVars);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.profile);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.profile);
});
```

## ListMyProfiles
You can execute the `ListMyProfiles` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
listMyProfiles(options?: ExecuteQueryOptions): QueryPromise<ListMyProfilesData, undefined>;

interface ListMyProfilesRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyProfilesData, undefined>;
}
export const listMyProfilesRef: ListMyProfilesRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
listMyProfiles(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyProfilesData, undefined>;

interface ListMyProfilesRef {
  ...
  (dc: DataConnect): QueryRef<ListMyProfilesData, undefined>;
}
export const listMyProfilesRef: ListMyProfilesRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the listMyProfilesRef:
```typescript
const name = listMyProfilesRef.operationName;
console.log(name);
```

### Variables
The `ListMyProfiles` query has no variables.
### Return Type
Recall that executing the `ListMyProfiles` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `ListMyProfilesData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface ListMyProfilesData {
  profiles: ({
    name: string;
    isDefault?: boolean | null;
  })[];
}
```
### Using `ListMyProfiles`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, listMyProfiles } from '@dataconnect/generated';


// Call the `listMyProfiles()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await listMyProfiles();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await listMyProfiles(dataConnect);

console.log(data.profiles);

// Or, you can use the `Promise` API.
listMyProfiles().then((response) => {
  const data = response.data;
  console.log(data.profiles);
});
```

### Using `ListMyProfiles`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, listMyProfilesRef } from '@dataconnect/generated';


// Call the `listMyProfilesRef()` function to get a reference to the query.
const ref = listMyProfilesRef();

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = listMyProfilesRef(dataConnect);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.profiles);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.profiles);
});
```

## GetShellConfig
You can execute the `GetShellConfig` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
getShellConfig(vars: GetShellConfigVariables, options?: ExecuteQueryOptions): QueryPromise<GetShellConfigData, GetShellConfigVariables>;

interface GetShellConfigRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetShellConfigVariables): QueryRef<GetShellConfigData, GetShellConfigVariables>;
}
export const getShellConfigRef: GetShellConfigRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
getShellConfig(dc: DataConnect, vars: GetShellConfigVariables, options?: ExecuteQueryOptions): QueryPromise<GetShellConfigData, GetShellConfigVariables>;

interface GetShellConfigRef {
  ...
  (dc: DataConnect, vars: GetShellConfigVariables): QueryRef<GetShellConfigData, GetShellConfigVariables>;
}
export const getShellConfigRef: GetShellConfigRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the getShellConfigRef:
```typescript
const name = getShellConfigRef.operationName;
console.log(name);
```

### Variables
The `GetShellConfig` query requires an argument of type `GetShellConfigVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface GetShellConfigVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `GetShellConfig` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `GetShellConfigData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface GetShellConfigData {
  shellConfig?: {
    promptTemplate: string;
    aliasMapping: string;
  };
}
```
### Using `GetShellConfig`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, getShellConfig, GetShellConfigVariables } from '@dataconnect/generated';

// The `GetShellConfig` query requires an argument of type `GetShellConfigVariables`:
const getShellConfigVars: GetShellConfigVariables = {
  id: ..., 
};

// Call the `getShellConfig()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await getShellConfig(getShellConfigVars);
// Variables can be defined inline as well.
const { data } = await getShellConfig({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await getShellConfig(dataConnect, getShellConfigVars);

console.log(data.shellConfig);

// Or, you can use the `Promise` API.
getShellConfig(getShellConfigVars).then((response) => {
  const data = response.data;
  console.log(data.shellConfig);
});
```

### Using `GetShellConfig`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, getShellConfigRef, GetShellConfigVariables } from '@dataconnect/generated';

// The `GetShellConfig` query requires an argument of type `GetShellConfigVariables`:
const getShellConfigVars: GetShellConfigVariables = {
  id: ..., 
};

// Call the `getShellConfigRef()` function to get a reference to the query.
const ref = getShellConfigRef(getShellConfigVars);
// Variables can be defined inline as well.
const ref = getShellConfigRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = getShellConfigRef(dataConnect, getShellConfigVars);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.shellConfig);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.shellConfig);
});
```

## ListMyShellConfigs
You can execute the `ListMyShellConfigs` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
listMyShellConfigs(options?: ExecuteQueryOptions): QueryPromise<ListMyShellConfigsData, undefined>;

interface ListMyShellConfigsRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyShellConfigsData, undefined>;
}
export const listMyShellConfigsRef: ListMyShellConfigsRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
listMyShellConfigs(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyShellConfigsData, undefined>;

interface ListMyShellConfigsRef {
  ...
  (dc: DataConnect): QueryRef<ListMyShellConfigsData, undefined>;
}
export const listMyShellConfigsRef: ListMyShellConfigsRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the listMyShellConfigsRef:
```typescript
const name = listMyShellConfigsRef.operationName;
console.log(name);
```

### Variables
The `ListMyShellConfigs` query has no variables.
### Return Type
Recall that executing the `ListMyShellConfigs` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `ListMyShellConfigsData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface ListMyShellConfigsData {
  shellConfigs: ({
    promptTemplate: string;
  })[];
}
```
### Using `ListMyShellConfigs`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, listMyShellConfigs } from '@dataconnect/generated';


// Call the `listMyShellConfigs()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await listMyShellConfigs();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await listMyShellConfigs(dataConnect);

console.log(data.shellConfigs);

// Or, you can use the `Promise` API.
listMyShellConfigs().then((response) => {
  const data = response.data;
  console.log(data.shellConfigs);
});
```

### Using `ListMyShellConfigs`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, listMyShellConfigsRef } from '@dataconnect/generated';


// Call the `listMyShellConfigsRef()` function to get a reference to the query.
const ref = listMyShellConfigsRef();

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = listMyShellConfigsRef(dataConnect);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.shellConfigs);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.shellConfigs);
});
```

## GetCommandHistory
You can execute the `GetCommandHistory` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
getCommandHistory(vars: GetCommandHistoryVariables, options?: ExecuteQueryOptions): QueryPromise<GetCommandHistoryData, GetCommandHistoryVariables>;

interface GetCommandHistoryRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetCommandHistoryVariables): QueryRef<GetCommandHistoryData, GetCommandHistoryVariables>;
}
export const getCommandHistoryRef: GetCommandHistoryRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
getCommandHistory(dc: DataConnect, vars: GetCommandHistoryVariables, options?: ExecuteQueryOptions): QueryPromise<GetCommandHistoryData, GetCommandHistoryVariables>;

interface GetCommandHistoryRef {
  ...
  (dc: DataConnect, vars: GetCommandHistoryVariables): QueryRef<GetCommandHistoryData, GetCommandHistoryVariables>;
}
export const getCommandHistoryRef: GetCommandHistoryRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the getCommandHistoryRef:
```typescript
const name = getCommandHistoryRef.operationName;
console.log(name);
```

### Variables
The `GetCommandHistory` query requires an argument of type `GetCommandHistoryVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface GetCommandHistoryVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `GetCommandHistory` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `GetCommandHistoryData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface GetCommandHistoryData {
  commandHistory?: {
    commandString: string;
    timestamp: TimestampString;
  };
}
```
### Using `GetCommandHistory`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, getCommandHistory, GetCommandHistoryVariables } from '@dataconnect/generated';

// The `GetCommandHistory` query requires an argument of type `GetCommandHistoryVariables`:
const getCommandHistoryVars: GetCommandHistoryVariables = {
  id: ..., 
};

// Call the `getCommandHistory()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await getCommandHistory(getCommandHistoryVars);
// Variables can be defined inline as well.
const { data } = await getCommandHistory({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await getCommandHistory(dataConnect, getCommandHistoryVars);

console.log(data.commandHistory);

// Or, you can use the `Promise` API.
getCommandHistory(getCommandHistoryVars).then((response) => {
  const data = response.data;
  console.log(data.commandHistory);
});
```

### Using `GetCommandHistory`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, getCommandHistoryRef, GetCommandHistoryVariables } from '@dataconnect/generated';

// The `GetCommandHistory` query requires an argument of type `GetCommandHistoryVariables`:
const getCommandHistoryVars: GetCommandHistoryVariables = {
  id: ..., 
};

// Call the `getCommandHistoryRef()` function to get a reference to the query.
const ref = getCommandHistoryRef(getCommandHistoryVars);
// Variables can be defined inline as well.
const ref = getCommandHistoryRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = getCommandHistoryRef(dataConnect, getCommandHistoryVars);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.commandHistory);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.commandHistory);
});
```

## ListMyCommandHistory
You can execute the `ListMyCommandHistory` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
listMyCommandHistory(options?: ExecuteQueryOptions): QueryPromise<ListMyCommandHistoryData, undefined>;

interface ListMyCommandHistoryRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyCommandHistoryData, undefined>;
}
export const listMyCommandHistoryRef: ListMyCommandHistoryRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
listMyCommandHistory(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyCommandHistoryData, undefined>;

interface ListMyCommandHistoryRef {
  ...
  (dc: DataConnect): QueryRef<ListMyCommandHistoryData, undefined>;
}
export const listMyCommandHistoryRef: ListMyCommandHistoryRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the listMyCommandHistoryRef:
```typescript
const name = listMyCommandHistoryRef.operationName;
console.log(name);
```

### Variables
The `ListMyCommandHistory` query has no variables.
### Return Type
Recall that executing the `ListMyCommandHistory` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `ListMyCommandHistoryData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface ListMyCommandHistoryData {
  commandHistories: ({
    commandString: string;
  })[];
}
```
### Using `ListMyCommandHistory`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, listMyCommandHistory } from '@dataconnect/generated';


// Call the `listMyCommandHistory()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await listMyCommandHistory();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await listMyCommandHistory(dataConnect);

console.log(data.commandHistories);

// Or, you can use the `Promise` API.
listMyCommandHistory().then((response) => {
  const data = response.data;
  console.log(data.commandHistories);
});
```

### Using `ListMyCommandHistory`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, listMyCommandHistoryRef } from '@dataconnect/generated';


// Call the `listMyCommandHistoryRef()` function to get a reference to the query.
const ref = listMyCommandHistoryRef();

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = listMyCommandHistoryRef(dataConnect);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.commandHistories);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.commandHistories);
});
```

## GetTerminalTheme
You can execute the `GetTerminalTheme` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
getTerminalTheme(vars: GetTerminalThemeVariables, options?: ExecuteQueryOptions): QueryPromise<GetTerminalThemeData, GetTerminalThemeVariables>;

interface GetTerminalThemeRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetTerminalThemeVariables): QueryRef<GetTerminalThemeData, GetTerminalThemeVariables>;
}
export const getTerminalThemeRef: GetTerminalThemeRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
getTerminalTheme(dc: DataConnect, vars: GetTerminalThemeVariables, options?: ExecuteQueryOptions): QueryPromise<GetTerminalThemeData, GetTerminalThemeVariables>;

interface GetTerminalThemeRef {
  ...
  (dc: DataConnect, vars: GetTerminalThemeVariables): QueryRef<GetTerminalThemeData, GetTerminalThemeVariables>;
}
export const getTerminalThemeRef: GetTerminalThemeRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the getTerminalThemeRef:
```typescript
const name = getTerminalThemeRef.operationName;
console.log(name);
```

### Variables
The `GetTerminalTheme` query requires an argument of type `GetTerminalThemeVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface GetTerminalThemeVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `GetTerminalTheme` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `GetTerminalThemeData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface GetTerminalThemeData {
  terminalTheme?: {
    name: string;
    colorPalette: string;
  };
}
```
### Using `GetTerminalTheme`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, getTerminalTheme, GetTerminalThemeVariables } from '@dataconnect/generated';

// The `GetTerminalTheme` query requires an argument of type `GetTerminalThemeVariables`:
const getTerminalThemeVars: GetTerminalThemeVariables = {
  id: ..., 
};

// Call the `getTerminalTheme()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await getTerminalTheme(getTerminalThemeVars);
// Variables can be defined inline as well.
const { data } = await getTerminalTheme({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await getTerminalTheme(dataConnect, getTerminalThemeVars);

console.log(data.terminalTheme);

// Or, you can use the `Promise` API.
getTerminalTheme(getTerminalThemeVars).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme);
});
```

### Using `GetTerminalTheme`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, getTerminalThemeRef, GetTerminalThemeVariables } from '@dataconnect/generated';

// The `GetTerminalTheme` query requires an argument of type `GetTerminalThemeVariables`:
const getTerminalThemeVars: GetTerminalThemeVariables = {
  id: ..., 
};

// Call the `getTerminalThemeRef()` function to get a reference to the query.
const ref = getTerminalThemeRef(getTerminalThemeVars);
// Variables can be defined inline as well.
const ref = getTerminalThemeRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = getTerminalThemeRef(dataConnect, getTerminalThemeVars);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.terminalTheme);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme);
});
```

## ListMyTerminalThemes
You can execute the `ListMyTerminalThemes` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
listMyTerminalThemes(options?: ExecuteQueryOptions): QueryPromise<ListMyTerminalThemesData, undefined>;

interface ListMyTerminalThemesRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyTerminalThemesData, undefined>;
}
export const listMyTerminalThemesRef: ListMyTerminalThemesRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
listMyTerminalThemes(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyTerminalThemesData, undefined>;

interface ListMyTerminalThemesRef {
  ...
  (dc: DataConnect): QueryRef<ListMyTerminalThemesData, undefined>;
}
export const listMyTerminalThemesRef: ListMyTerminalThemesRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the listMyTerminalThemesRef:
```typescript
const name = listMyTerminalThemesRef.operationName;
console.log(name);
```

### Variables
The `ListMyTerminalThemes` query has no variables.
### Return Type
Recall that executing the `ListMyTerminalThemes` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `ListMyTerminalThemesData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface ListMyTerminalThemesData {
  terminalThemes: ({
    name: string;
  })[];
}
```
### Using `ListMyTerminalThemes`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, listMyTerminalThemes } from '@dataconnect/generated';


// Call the `listMyTerminalThemes()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await listMyTerminalThemes();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await listMyTerminalThemes(dataConnect);

console.log(data.terminalThemes);

// Or, you can use the `Promise` API.
listMyTerminalThemes().then((response) => {
  const data = response.data;
  console.log(data.terminalThemes);
});
```

### Using `ListMyTerminalThemes`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, listMyTerminalThemesRef } from '@dataconnect/generated';


// Call the `listMyTerminalThemesRef()` function to get a reference to the query.
const ref = listMyTerminalThemesRef();

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = listMyTerminalThemesRef(dataConnect);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.terminalThemes);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.terminalThemes);
});
```

## GetDeviceSync
You can execute the `GetDeviceSync` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
getDeviceSync(vars: GetDeviceSyncVariables, options?: ExecuteQueryOptions): QueryPromise<GetDeviceSyncData, GetDeviceSyncVariables>;

interface GetDeviceSyncRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetDeviceSyncVariables): QueryRef<GetDeviceSyncData, GetDeviceSyncVariables>;
}
export const getDeviceSyncRef: GetDeviceSyncRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
getDeviceSync(dc: DataConnect, vars: GetDeviceSyncVariables, options?: ExecuteQueryOptions): QueryPromise<GetDeviceSyncData, GetDeviceSyncVariables>;

interface GetDeviceSyncRef {
  ...
  (dc: DataConnect, vars: GetDeviceSyncVariables): QueryRef<GetDeviceSyncData, GetDeviceSyncVariables>;
}
export const getDeviceSyncRef: GetDeviceSyncRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the getDeviceSyncRef:
```typescript
const name = getDeviceSyncRef.operationName;
console.log(name);
```

### Variables
The `GetDeviceSync` query requires an argument of type `GetDeviceSyncVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface GetDeviceSyncVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `GetDeviceSync` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `GetDeviceSyncData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface GetDeviceSyncData {
  deviceSync?: {
    deviceName: string;
    lastSeen: TimestampString;
  };
}
```
### Using `GetDeviceSync`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, getDeviceSync, GetDeviceSyncVariables } from '@dataconnect/generated';

// The `GetDeviceSync` query requires an argument of type `GetDeviceSyncVariables`:
const getDeviceSyncVars: GetDeviceSyncVariables = {
  id: ..., 
};

// Call the `getDeviceSync()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await getDeviceSync(getDeviceSyncVars);
// Variables can be defined inline as well.
const { data } = await getDeviceSync({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await getDeviceSync(dataConnect, getDeviceSyncVars);

console.log(data.deviceSync);

// Or, you can use the `Promise` API.
getDeviceSync(getDeviceSyncVars).then((response) => {
  const data = response.data;
  console.log(data.deviceSync);
});
```

### Using `GetDeviceSync`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, getDeviceSyncRef, GetDeviceSyncVariables } from '@dataconnect/generated';

// The `GetDeviceSync` query requires an argument of type `GetDeviceSyncVariables`:
const getDeviceSyncVars: GetDeviceSyncVariables = {
  id: ..., 
};

// Call the `getDeviceSyncRef()` function to get a reference to the query.
const ref = getDeviceSyncRef(getDeviceSyncVars);
// Variables can be defined inline as well.
const ref = getDeviceSyncRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = getDeviceSyncRef(dataConnect, getDeviceSyncVars);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.deviceSync);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.deviceSync);
});
```

## ListMyDevices
You can execute the `ListMyDevices` query using the following action shortcut function, or by calling `executeQuery()` after calling the following `QueryRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
listMyDevices(options?: ExecuteQueryOptions): QueryPromise<ListMyDevicesData, undefined>;

interface ListMyDevicesRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyDevicesData, undefined>;
}
export const listMyDevicesRef: ListMyDevicesRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `QueryRef` function.
```typescript
listMyDevices(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyDevicesData, undefined>;

interface ListMyDevicesRef {
  ...
  (dc: DataConnect): QueryRef<ListMyDevicesData, undefined>;
}
export const listMyDevicesRef: ListMyDevicesRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the listMyDevicesRef:
```typescript
const name = listMyDevicesRef.operationName;
console.log(name);
```

### Variables
The `ListMyDevices` query has no variables.
### Return Type
Recall that executing the `ListMyDevices` query returns a `QueryPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `ListMyDevicesData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface ListMyDevicesData {
  deviceSyncs: ({
    deviceName: string;
    lastSeen: TimestampString;
  })[];
}
```
### Using `ListMyDevices`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, listMyDevices } from '@dataconnect/generated';


// Call the `listMyDevices()` function to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await listMyDevices();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await listMyDevices(dataConnect);

console.log(data.deviceSyncs);

// Or, you can use the `Promise` API.
listMyDevices().then((response) => {
  const data = response.data;
  console.log(data.deviceSyncs);
});
```

### Using `ListMyDevices`'s `QueryRef` function

```typescript
import { getDataConnect, executeQuery } from 'firebase/data-connect';
import { connectorConfig, listMyDevicesRef } from '@dataconnect/generated';


// Call the `listMyDevicesRef()` function to get a reference to the query.
const ref = listMyDevicesRef();

// You can also pass in a `DataConnect` instance to the `QueryRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = listMyDevicesRef(dataConnect);

// Call `executeQuery()` on the reference to execute the query.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeQuery(ref);

console.log(data.deviceSyncs);

// Or, you can use the `Promise` API.
executeQuery(ref).then((response) => {
  const data = response.data;
  console.log(data.deviceSyncs);
});
```

# Mutations

There are two ways to execute a Data Connect Mutation using the generated Web SDK:
- Using a Mutation Reference function, which returns a `MutationRef`
  - The `MutationRef` can be used as an argument to `executeMutation()`, which will execute the Mutation and return a `MutationPromise`
- Using an action shortcut function, which returns a `MutationPromise`
  - Calling the action shortcut function will execute the Mutation and return a `MutationPromise`

The following is true for both the action shortcut function and the `MutationRef` function:
- The `MutationPromise` returned will resolve to the result of the Mutation once it has finished executing
- If the Mutation accepts arguments, both the action shortcut function and the `MutationRef` function accept a single argument: an object that contains all the required variables (and the optional variables) for the Mutation
- Both functions can be called with or without passing in a `DataConnect` instance as an argument. If no `DataConnect` argument is passed in, then the generated SDK will call `getDataConnect(connectorConfig)` behind the scenes for you.

Below are examples of how to use the `example` connector's generated functions to execute each mutation. You can also follow the examples from the [Data Connect documentation](https://firebase.google.com/docs/data-connect/web-sdk#using-mutations).

## CreateUser
You can execute the `CreateUser` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
createUser(): MutationPromise<CreateUserData, undefined>;

interface CreateUserRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): MutationRef<CreateUserData, undefined>;
}
export const createUserRef: CreateUserRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
createUser(dc: DataConnect): MutationPromise<CreateUserData, undefined>;

interface CreateUserRef {
  ...
  (dc: DataConnect): MutationRef<CreateUserData, undefined>;
}
export const createUserRef: CreateUserRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the createUserRef:
```typescript
const name = createUserRef.operationName;
console.log(name);
```

### Variables
The `CreateUser` mutation has no variables.
### Return Type
Recall that executing the `CreateUser` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `CreateUserData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface CreateUserData {
  user_insert: User_Key;
}
```
### Using `CreateUser`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, createUser } from '@dataconnect/generated';


// Call the `createUser()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await createUser();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await createUser(dataConnect);

console.log(data.user_insert);

// Or, you can use the `Promise` API.
createUser().then((response) => {
  const data = response.data;
  console.log(data.user_insert);
});
```

### Using `CreateUser`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, createUserRef } from '@dataconnect/generated';


// Call the `createUserRef()` function to get a reference to the mutation.
const ref = createUserRef();

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = createUserRef(dataConnect);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.user_insert);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.user_insert);
});
```

## UpdateUser
You can execute the `UpdateUser` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
updateUser(vars: UpdateUserVariables): MutationPromise<UpdateUserData, UpdateUserVariables>;

interface UpdateUserRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateUserVariables): MutationRef<UpdateUserData, UpdateUserVariables>;
}
export const updateUserRef: UpdateUserRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
updateUser(dc: DataConnect, vars: UpdateUserVariables): MutationPromise<UpdateUserData, UpdateUserVariables>;

interface UpdateUserRef {
  ...
  (dc: DataConnect, vars: UpdateUserVariables): MutationRef<UpdateUserData, UpdateUserVariables>;
}
export const updateUserRef: UpdateUserRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the updateUserRef:
```typescript
const name = updateUserRef.operationName;
console.log(name);
```

### Variables
The `UpdateUser` mutation requires an argument of type `UpdateUserVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface UpdateUserVariables {
  username: string;
}
```
### Return Type
Recall that executing the `UpdateUser` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `UpdateUserData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface UpdateUserData {
  user_update?: User_Key | null;
}
```
### Using `UpdateUser`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, updateUser, UpdateUserVariables } from '@dataconnect/generated';

// The `UpdateUser` mutation requires an argument of type `UpdateUserVariables`:
const updateUserVars: UpdateUserVariables = {
  username: ..., 
};

// Call the `updateUser()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await updateUser(updateUserVars);
// Variables can be defined inline as well.
const { data } = await updateUser({ username: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await updateUser(dataConnect, updateUserVars);

console.log(data.user_update);

// Or, you can use the `Promise` API.
updateUser(updateUserVars).then((response) => {
  const data = response.data;
  console.log(data.user_update);
});
```

### Using `UpdateUser`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, updateUserRef, UpdateUserVariables } from '@dataconnect/generated';

// The `UpdateUser` mutation requires an argument of type `UpdateUserVariables`:
const updateUserVars: UpdateUserVariables = {
  username: ..., 
};

// Call the `updateUserRef()` function to get a reference to the mutation.
const ref = updateUserRef(updateUserVars);
// Variables can be defined inline as well.
const ref = updateUserRef({ username: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = updateUserRef(dataConnect, updateUserVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.user_update);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.user_update);
});
```

## DeleteUser
You can execute the `DeleteUser` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
deleteUser(): MutationPromise<DeleteUserData, undefined>;

interface DeleteUserRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (): MutationRef<DeleteUserData, undefined>;
}
export const deleteUserRef: DeleteUserRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
deleteUser(dc: DataConnect): MutationPromise<DeleteUserData, undefined>;

interface DeleteUserRef {
  ...
  (dc: DataConnect): MutationRef<DeleteUserData, undefined>;
}
export const deleteUserRef: DeleteUserRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the deleteUserRef:
```typescript
const name = deleteUserRef.operationName;
console.log(name);
```

### Variables
The `DeleteUser` mutation has no variables.
### Return Type
Recall that executing the `DeleteUser` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `DeleteUserData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface DeleteUserData {
  user_delete?: User_Key | null;
}
```
### Using `DeleteUser`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, deleteUser } from '@dataconnect/generated';


// Call the `deleteUser()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await deleteUser();

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await deleteUser(dataConnect);

console.log(data.user_delete);

// Or, you can use the `Promise` API.
deleteUser().then((response) => {
  const data = response.data;
  console.log(data.user_delete);
});
```

### Using `DeleteUser`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, deleteUserRef } from '@dataconnect/generated';


// Call the `deleteUserRef()` function to get a reference to the mutation.
const ref = deleteUserRef();

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = deleteUserRef(dataConnect);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.user_delete);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.user_delete);
});
```

## CreateProfile
You can execute the `CreateProfile` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
createProfile(vars: CreateProfileVariables): MutationPromise<CreateProfileData, CreateProfileVariables>;

interface CreateProfileRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: CreateProfileVariables): MutationRef<CreateProfileData, CreateProfileVariables>;
}
export const createProfileRef: CreateProfileRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
createProfile(dc: DataConnect, vars: CreateProfileVariables): MutationPromise<CreateProfileData, CreateProfileVariables>;

interface CreateProfileRef {
  ...
  (dc: DataConnect, vars: CreateProfileVariables): MutationRef<CreateProfileData, CreateProfileVariables>;
}
export const createProfileRef: CreateProfileRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the createProfileRef:
```typescript
const name = createProfileRef.operationName;
console.log(name);
```

### Variables
The `CreateProfile` mutation requires an argument of type `CreateProfileVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface CreateProfileVariables {
  name: string;
  metadata?: string | null;
}
```
### Return Type
Recall that executing the `CreateProfile` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `CreateProfileData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface CreateProfileData {
  profile_insert: Profile_Key;
}
```
### Using `CreateProfile`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, createProfile, CreateProfileVariables } from '@dataconnect/generated';

// The `CreateProfile` mutation requires an argument of type `CreateProfileVariables`:
const createProfileVars: CreateProfileVariables = {
  name: ..., 
  metadata: ..., // optional
};

// Call the `createProfile()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await createProfile(createProfileVars);
// Variables can be defined inline as well.
const { data } = await createProfile({ name: ..., metadata: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await createProfile(dataConnect, createProfileVars);

console.log(data.profile_insert);

// Or, you can use the `Promise` API.
createProfile(createProfileVars).then((response) => {
  const data = response.data;
  console.log(data.profile_insert);
});
```

### Using `CreateProfile`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, createProfileRef, CreateProfileVariables } from '@dataconnect/generated';

// The `CreateProfile` mutation requires an argument of type `CreateProfileVariables`:
const createProfileVars: CreateProfileVariables = {
  name: ..., 
  metadata: ..., // optional
};

// Call the `createProfileRef()` function to get a reference to the mutation.
const ref = createProfileRef(createProfileVars);
// Variables can be defined inline as well.
const ref = createProfileRef({ name: ..., metadata: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = createProfileRef(dataConnect, createProfileVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.profile_insert);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.profile_insert);
});
```

## UpdateProfile
You can execute the `UpdateProfile` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
updateProfile(vars: UpdateProfileVariables): MutationPromise<UpdateProfileData, UpdateProfileVariables>;

interface UpdateProfileRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateProfileVariables): MutationRef<UpdateProfileData, UpdateProfileVariables>;
}
export const updateProfileRef: UpdateProfileRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
updateProfile(dc: DataConnect, vars: UpdateProfileVariables): MutationPromise<UpdateProfileData, UpdateProfileVariables>;

interface UpdateProfileRef {
  ...
  (dc: DataConnect, vars: UpdateProfileVariables): MutationRef<UpdateProfileData, UpdateProfileVariables>;
}
export const updateProfileRef: UpdateProfileRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the updateProfileRef:
```typescript
const name = updateProfileRef.operationName;
console.log(name);
```

### Variables
The `UpdateProfile` mutation requires an argument of type `UpdateProfileVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface UpdateProfileVariables {
  id: UUIDString;
  name?: string | null;
}
```
### Return Type
Recall that executing the `UpdateProfile` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `UpdateProfileData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface UpdateProfileData {
  profile_update?: Profile_Key | null;
}
```
### Using `UpdateProfile`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, updateProfile, UpdateProfileVariables } from '@dataconnect/generated';

// The `UpdateProfile` mutation requires an argument of type `UpdateProfileVariables`:
const updateProfileVars: UpdateProfileVariables = {
  id: ..., 
  name: ..., // optional
};

// Call the `updateProfile()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await updateProfile(updateProfileVars);
// Variables can be defined inline as well.
const { data } = await updateProfile({ id: ..., name: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await updateProfile(dataConnect, updateProfileVars);

console.log(data.profile_update);

// Or, you can use the `Promise` API.
updateProfile(updateProfileVars).then((response) => {
  const data = response.data;
  console.log(data.profile_update);
});
```

### Using `UpdateProfile`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, updateProfileRef, UpdateProfileVariables } from '@dataconnect/generated';

// The `UpdateProfile` mutation requires an argument of type `UpdateProfileVariables`:
const updateProfileVars: UpdateProfileVariables = {
  id: ..., 
  name: ..., // optional
};

// Call the `updateProfileRef()` function to get a reference to the mutation.
const ref = updateProfileRef(updateProfileVars);
// Variables can be defined inline as well.
const ref = updateProfileRef({ id: ..., name: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = updateProfileRef(dataConnect, updateProfileVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.profile_update);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.profile_update);
});
```

## DeleteProfile
You can execute the `DeleteProfile` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
deleteProfile(vars: DeleteProfileVariables): MutationPromise<DeleteProfileData, DeleteProfileVariables>;

interface DeleteProfileRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteProfileVariables): MutationRef<DeleteProfileData, DeleteProfileVariables>;
}
export const deleteProfileRef: DeleteProfileRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
deleteProfile(dc: DataConnect, vars: DeleteProfileVariables): MutationPromise<DeleteProfileData, DeleteProfileVariables>;

interface DeleteProfileRef {
  ...
  (dc: DataConnect, vars: DeleteProfileVariables): MutationRef<DeleteProfileData, DeleteProfileVariables>;
}
export const deleteProfileRef: DeleteProfileRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the deleteProfileRef:
```typescript
const name = deleteProfileRef.operationName;
console.log(name);
```

### Variables
The `DeleteProfile` mutation requires an argument of type `DeleteProfileVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface DeleteProfileVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `DeleteProfile` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `DeleteProfileData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface DeleteProfileData {
  profile_delete?: Profile_Key | null;
}
```
### Using `DeleteProfile`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, deleteProfile, DeleteProfileVariables } from '@dataconnect/generated';

// The `DeleteProfile` mutation requires an argument of type `DeleteProfileVariables`:
const deleteProfileVars: DeleteProfileVariables = {
  id: ..., 
};

// Call the `deleteProfile()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await deleteProfile(deleteProfileVars);
// Variables can be defined inline as well.
const { data } = await deleteProfile({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await deleteProfile(dataConnect, deleteProfileVars);

console.log(data.profile_delete);

// Or, you can use the `Promise` API.
deleteProfile(deleteProfileVars).then((response) => {
  const data = response.data;
  console.log(data.profile_delete);
});
```

### Using `DeleteProfile`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, deleteProfileRef, DeleteProfileVariables } from '@dataconnect/generated';

// The `DeleteProfile` mutation requires an argument of type `DeleteProfileVariables`:
const deleteProfileVars: DeleteProfileVariables = {
  id: ..., 
};

// Call the `deleteProfileRef()` function to get a reference to the mutation.
const ref = deleteProfileRef(deleteProfileVars);
// Variables can be defined inline as well.
const ref = deleteProfileRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = deleteProfileRef(dataConnect, deleteProfileVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.profile_delete);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.profile_delete);
});
```

## CreateShellConfig
You can execute the `CreateShellConfig` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
createShellConfig(vars: CreateShellConfigVariables): MutationPromise<CreateShellConfigData, CreateShellConfigVariables>;

interface CreateShellConfigRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: CreateShellConfigVariables): MutationRef<CreateShellConfigData, CreateShellConfigVariables>;
}
export const createShellConfigRef: CreateShellConfigRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
createShellConfig(dc: DataConnect, vars: CreateShellConfigVariables): MutationPromise<CreateShellConfigData, CreateShellConfigVariables>;

interface CreateShellConfigRef {
  ...
  (dc: DataConnect, vars: CreateShellConfigVariables): MutationRef<CreateShellConfigData, CreateShellConfigVariables>;
}
export const createShellConfigRef: CreateShellConfigRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the createShellConfigRef:
```typescript
const name = createShellConfigRef.operationName;
console.log(name);
```

### Variables
The `CreateShellConfig` mutation requires an argument of type `CreateShellConfigVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface CreateShellConfigVariables {
  profileId: UUIDString;
  prompt: string;
  alias: string;
}
```
### Return Type
Recall that executing the `CreateShellConfig` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `CreateShellConfigData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface CreateShellConfigData {
  shellConfig_insert: ShellConfig_Key;
}
```
### Using `CreateShellConfig`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, createShellConfig, CreateShellConfigVariables } from '@dataconnect/generated';

// The `CreateShellConfig` mutation requires an argument of type `CreateShellConfigVariables`:
const createShellConfigVars: CreateShellConfigVariables = {
  profileId: ..., 
  prompt: ..., 
  alias: ..., 
};

// Call the `createShellConfig()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await createShellConfig(createShellConfigVars);
// Variables can be defined inline as well.
const { data } = await createShellConfig({ profileId: ..., prompt: ..., alias: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await createShellConfig(dataConnect, createShellConfigVars);

console.log(data.shellConfig_insert);

// Or, you can use the `Promise` API.
createShellConfig(createShellConfigVars).then((response) => {
  const data = response.data;
  console.log(data.shellConfig_insert);
});
```

### Using `CreateShellConfig`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, createShellConfigRef, CreateShellConfigVariables } from '@dataconnect/generated';

// The `CreateShellConfig` mutation requires an argument of type `CreateShellConfigVariables`:
const createShellConfigVars: CreateShellConfigVariables = {
  profileId: ..., 
  prompt: ..., 
  alias: ..., 
};

// Call the `createShellConfigRef()` function to get a reference to the mutation.
const ref = createShellConfigRef(createShellConfigVars);
// Variables can be defined inline as well.
const ref = createShellConfigRef({ profileId: ..., prompt: ..., alias: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = createShellConfigRef(dataConnect, createShellConfigVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.shellConfig_insert);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.shellConfig_insert);
});
```

## UpdateShellConfig
You can execute the `UpdateShellConfig` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
updateShellConfig(vars: UpdateShellConfigVariables): MutationPromise<UpdateShellConfigData, UpdateShellConfigVariables>;

interface UpdateShellConfigRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateShellConfigVariables): MutationRef<UpdateShellConfigData, UpdateShellConfigVariables>;
}
export const updateShellConfigRef: UpdateShellConfigRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
updateShellConfig(dc: DataConnect, vars: UpdateShellConfigVariables): MutationPromise<UpdateShellConfigData, UpdateShellConfigVariables>;

interface UpdateShellConfigRef {
  ...
  (dc: DataConnect, vars: UpdateShellConfigVariables): MutationRef<UpdateShellConfigData, UpdateShellConfigVariables>;
}
export const updateShellConfigRef: UpdateShellConfigRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the updateShellConfigRef:
```typescript
const name = updateShellConfigRef.operationName;
console.log(name);
```

### Variables
The `UpdateShellConfig` mutation requires an argument of type `UpdateShellConfigVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface UpdateShellConfigVariables {
  id: UUIDString;
  prompt?: string | null;
}
```
### Return Type
Recall that executing the `UpdateShellConfig` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `UpdateShellConfigData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface UpdateShellConfigData {
  shellConfig_update?: ShellConfig_Key | null;
}
```
### Using `UpdateShellConfig`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, updateShellConfig, UpdateShellConfigVariables } from '@dataconnect/generated';

// The `UpdateShellConfig` mutation requires an argument of type `UpdateShellConfigVariables`:
const updateShellConfigVars: UpdateShellConfigVariables = {
  id: ..., 
  prompt: ..., // optional
};

// Call the `updateShellConfig()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await updateShellConfig(updateShellConfigVars);
// Variables can be defined inline as well.
const { data } = await updateShellConfig({ id: ..., prompt: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await updateShellConfig(dataConnect, updateShellConfigVars);

console.log(data.shellConfig_update);

// Or, you can use the `Promise` API.
updateShellConfig(updateShellConfigVars).then((response) => {
  const data = response.data;
  console.log(data.shellConfig_update);
});
```

### Using `UpdateShellConfig`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, updateShellConfigRef, UpdateShellConfigVariables } from '@dataconnect/generated';

// The `UpdateShellConfig` mutation requires an argument of type `UpdateShellConfigVariables`:
const updateShellConfigVars: UpdateShellConfigVariables = {
  id: ..., 
  prompt: ..., // optional
};

// Call the `updateShellConfigRef()` function to get a reference to the mutation.
const ref = updateShellConfigRef(updateShellConfigVars);
// Variables can be defined inline as well.
const ref = updateShellConfigRef({ id: ..., prompt: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = updateShellConfigRef(dataConnect, updateShellConfigVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.shellConfig_update);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.shellConfig_update);
});
```

## DeleteShellConfig
You can execute the `DeleteShellConfig` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
deleteShellConfig(vars: DeleteShellConfigVariables): MutationPromise<DeleteShellConfigData, DeleteShellConfigVariables>;

interface DeleteShellConfigRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteShellConfigVariables): MutationRef<DeleteShellConfigData, DeleteShellConfigVariables>;
}
export const deleteShellConfigRef: DeleteShellConfigRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
deleteShellConfig(dc: DataConnect, vars: DeleteShellConfigVariables): MutationPromise<DeleteShellConfigData, DeleteShellConfigVariables>;

interface DeleteShellConfigRef {
  ...
  (dc: DataConnect, vars: DeleteShellConfigVariables): MutationRef<DeleteShellConfigData, DeleteShellConfigVariables>;
}
export const deleteShellConfigRef: DeleteShellConfigRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the deleteShellConfigRef:
```typescript
const name = deleteShellConfigRef.operationName;
console.log(name);
```

### Variables
The `DeleteShellConfig` mutation requires an argument of type `DeleteShellConfigVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface DeleteShellConfigVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `DeleteShellConfig` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `DeleteShellConfigData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface DeleteShellConfigData {
  shellConfig_delete?: ShellConfig_Key | null;
}
```
### Using `DeleteShellConfig`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, deleteShellConfig, DeleteShellConfigVariables } from '@dataconnect/generated';

// The `DeleteShellConfig` mutation requires an argument of type `DeleteShellConfigVariables`:
const deleteShellConfigVars: DeleteShellConfigVariables = {
  id: ..., 
};

// Call the `deleteShellConfig()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await deleteShellConfig(deleteShellConfigVars);
// Variables can be defined inline as well.
const { data } = await deleteShellConfig({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await deleteShellConfig(dataConnect, deleteShellConfigVars);

console.log(data.shellConfig_delete);

// Or, you can use the `Promise` API.
deleteShellConfig(deleteShellConfigVars).then((response) => {
  const data = response.data;
  console.log(data.shellConfig_delete);
});
```

### Using `DeleteShellConfig`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, deleteShellConfigRef, DeleteShellConfigVariables } from '@dataconnect/generated';

// The `DeleteShellConfig` mutation requires an argument of type `DeleteShellConfigVariables`:
const deleteShellConfigVars: DeleteShellConfigVariables = {
  id: ..., 
};

// Call the `deleteShellConfigRef()` function to get a reference to the mutation.
const ref = deleteShellConfigRef(deleteShellConfigVars);
// Variables can be defined inline as well.
const ref = deleteShellConfigRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = deleteShellConfigRef(dataConnect, deleteShellConfigVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.shellConfig_delete);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.shellConfig_delete);
});
```

## AddCommandHistory
You can execute the `AddCommandHistory` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
addCommandHistory(vars: AddCommandHistoryVariables): MutationPromise<AddCommandHistoryData, AddCommandHistoryVariables>;

interface AddCommandHistoryRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: AddCommandHistoryVariables): MutationRef<AddCommandHistoryData, AddCommandHistoryVariables>;
}
export const addCommandHistoryRef: AddCommandHistoryRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
addCommandHistory(dc: DataConnect, vars: AddCommandHistoryVariables): MutationPromise<AddCommandHistoryData, AddCommandHistoryVariables>;

interface AddCommandHistoryRef {
  ...
  (dc: DataConnect, vars: AddCommandHistoryVariables): MutationRef<AddCommandHistoryData, AddCommandHistoryVariables>;
}
export const addCommandHistoryRef: AddCommandHistoryRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the addCommandHistoryRef:
```typescript
const name = addCommandHistoryRef.operationName;
console.log(name);
```

### Variables
The `AddCommandHistory` mutation requires an argument of type `AddCommandHistoryVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface AddCommandHistoryVariables {
  cmd: string;
  tag?: string | null;
}
```
### Return Type
Recall that executing the `AddCommandHistory` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `AddCommandHistoryData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface AddCommandHistoryData {
  commandHistory_insert: CommandHistory_Key;
}
```
### Using `AddCommandHistory`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, addCommandHistory, AddCommandHistoryVariables } from '@dataconnect/generated';

// The `AddCommandHistory` mutation requires an argument of type `AddCommandHistoryVariables`:
const addCommandHistoryVars: AddCommandHistoryVariables = {
  cmd: ..., 
  tag: ..., // optional
};

// Call the `addCommandHistory()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await addCommandHistory(addCommandHistoryVars);
// Variables can be defined inline as well.
const { data } = await addCommandHistory({ cmd: ..., tag: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await addCommandHistory(dataConnect, addCommandHistoryVars);

console.log(data.commandHistory_insert);

// Or, you can use the `Promise` API.
addCommandHistory(addCommandHistoryVars).then((response) => {
  const data = response.data;
  console.log(data.commandHistory_insert);
});
```

### Using `AddCommandHistory`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, addCommandHistoryRef, AddCommandHistoryVariables } from '@dataconnect/generated';

// The `AddCommandHistory` mutation requires an argument of type `AddCommandHistoryVariables`:
const addCommandHistoryVars: AddCommandHistoryVariables = {
  cmd: ..., 
  tag: ..., // optional
};

// Call the `addCommandHistoryRef()` function to get a reference to the mutation.
const ref = addCommandHistoryRef(addCommandHistoryVars);
// Variables can be defined inline as well.
const ref = addCommandHistoryRef({ cmd: ..., tag: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = addCommandHistoryRef(dataConnect, addCommandHistoryVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.commandHistory_insert);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.commandHistory_insert);
});
```

## DeleteCommandHistory
You can execute the `DeleteCommandHistory` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
deleteCommandHistory(vars: DeleteCommandHistoryVariables): MutationPromise<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;

interface DeleteCommandHistoryRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteCommandHistoryVariables): MutationRef<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;
}
export const deleteCommandHistoryRef: DeleteCommandHistoryRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
deleteCommandHistory(dc: DataConnect, vars: DeleteCommandHistoryVariables): MutationPromise<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;

interface DeleteCommandHistoryRef {
  ...
  (dc: DataConnect, vars: DeleteCommandHistoryVariables): MutationRef<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;
}
export const deleteCommandHistoryRef: DeleteCommandHistoryRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the deleteCommandHistoryRef:
```typescript
const name = deleteCommandHistoryRef.operationName;
console.log(name);
```

### Variables
The `DeleteCommandHistory` mutation requires an argument of type `DeleteCommandHistoryVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface DeleteCommandHistoryVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `DeleteCommandHistory` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `DeleteCommandHistoryData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface DeleteCommandHistoryData {
  commandHistory_delete?: CommandHistory_Key | null;
}
```
### Using `DeleteCommandHistory`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, deleteCommandHistory, DeleteCommandHistoryVariables } from '@dataconnect/generated';

// The `DeleteCommandHistory` mutation requires an argument of type `DeleteCommandHistoryVariables`:
const deleteCommandHistoryVars: DeleteCommandHistoryVariables = {
  id: ..., 
};

// Call the `deleteCommandHistory()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await deleteCommandHistory(deleteCommandHistoryVars);
// Variables can be defined inline as well.
const { data } = await deleteCommandHistory({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await deleteCommandHistory(dataConnect, deleteCommandHistoryVars);

console.log(data.commandHistory_delete);

// Or, you can use the `Promise` API.
deleteCommandHistory(deleteCommandHistoryVars).then((response) => {
  const data = response.data;
  console.log(data.commandHistory_delete);
});
```

### Using `DeleteCommandHistory`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, deleteCommandHistoryRef, DeleteCommandHistoryVariables } from '@dataconnect/generated';

// The `DeleteCommandHistory` mutation requires an argument of type `DeleteCommandHistoryVariables`:
const deleteCommandHistoryVars: DeleteCommandHistoryVariables = {
  id: ..., 
};

// Call the `deleteCommandHistoryRef()` function to get a reference to the mutation.
const ref = deleteCommandHistoryRef(deleteCommandHistoryVars);
// Variables can be defined inline as well.
const ref = deleteCommandHistoryRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = deleteCommandHistoryRef(dataConnect, deleteCommandHistoryVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.commandHistory_delete);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.commandHistory_delete);
});
```

## CreateTerminalTheme
You can execute the `CreateTerminalTheme` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
createTerminalTheme(vars: CreateTerminalThemeVariables): MutationPromise<CreateTerminalThemeData, CreateTerminalThemeVariables>;

interface CreateTerminalThemeRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: CreateTerminalThemeVariables): MutationRef<CreateTerminalThemeData, CreateTerminalThemeVariables>;
}
export const createTerminalThemeRef: CreateTerminalThemeRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
createTerminalTheme(dc: DataConnect, vars: CreateTerminalThemeVariables): MutationPromise<CreateTerminalThemeData, CreateTerminalThemeVariables>;

interface CreateTerminalThemeRef {
  ...
  (dc: DataConnect, vars: CreateTerminalThemeVariables): MutationRef<CreateTerminalThemeData, CreateTerminalThemeVariables>;
}
export const createTerminalThemeRef: CreateTerminalThemeRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the createTerminalThemeRef:
```typescript
const name = createTerminalThemeRef.operationName;
console.log(name);
```

### Variables
The `CreateTerminalTheme` mutation requires an argument of type `CreateTerminalThemeVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface CreateTerminalThemeVariables {
  name: string;
  palette: string;
  profileId: UUIDString;
}
```
### Return Type
Recall that executing the `CreateTerminalTheme` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `CreateTerminalThemeData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface CreateTerminalThemeData {
  terminalTheme_insert: TerminalTheme_Key;
}
```
### Using `CreateTerminalTheme`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, createTerminalTheme, CreateTerminalThemeVariables } from '@dataconnect/generated';

// The `CreateTerminalTheme` mutation requires an argument of type `CreateTerminalThemeVariables`:
const createTerminalThemeVars: CreateTerminalThemeVariables = {
  name: ..., 
  palette: ..., 
  profileId: ..., 
};

// Call the `createTerminalTheme()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await createTerminalTheme(createTerminalThemeVars);
// Variables can be defined inline as well.
const { data } = await createTerminalTheme({ name: ..., palette: ..., profileId: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await createTerminalTheme(dataConnect, createTerminalThemeVars);

console.log(data.terminalTheme_insert);

// Or, you can use the `Promise` API.
createTerminalTheme(createTerminalThemeVars).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme_insert);
});
```

### Using `CreateTerminalTheme`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, createTerminalThemeRef, CreateTerminalThemeVariables } from '@dataconnect/generated';

// The `CreateTerminalTheme` mutation requires an argument of type `CreateTerminalThemeVariables`:
const createTerminalThemeVars: CreateTerminalThemeVariables = {
  name: ..., 
  palette: ..., 
  profileId: ..., 
};

// Call the `createTerminalThemeRef()` function to get a reference to the mutation.
const ref = createTerminalThemeRef(createTerminalThemeVars);
// Variables can be defined inline as well.
const ref = createTerminalThemeRef({ name: ..., palette: ..., profileId: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = createTerminalThemeRef(dataConnect, createTerminalThemeVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.terminalTheme_insert);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme_insert);
});
```

## UpdateTerminalTheme
You can execute the `UpdateTerminalTheme` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
updateTerminalTheme(vars: UpdateTerminalThemeVariables): MutationPromise<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;

interface UpdateTerminalThemeRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateTerminalThemeVariables): MutationRef<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;
}
export const updateTerminalThemeRef: UpdateTerminalThemeRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
updateTerminalTheme(dc: DataConnect, vars: UpdateTerminalThemeVariables): MutationPromise<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;

interface UpdateTerminalThemeRef {
  ...
  (dc: DataConnect, vars: UpdateTerminalThemeVariables): MutationRef<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;
}
export const updateTerminalThemeRef: UpdateTerminalThemeRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the updateTerminalThemeRef:
```typescript
const name = updateTerminalThemeRef.operationName;
console.log(name);
```

### Variables
The `UpdateTerminalTheme` mutation requires an argument of type `UpdateTerminalThemeVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface UpdateTerminalThemeVariables {
  id: UUIDString;
  opacity?: number | null;
}
```
### Return Type
Recall that executing the `UpdateTerminalTheme` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `UpdateTerminalThemeData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface UpdateTerminalThemeData {
  terminalTheme_update?: TerminalTheme_Key | null;
}
```
### Using `UpdateTerminalTheme`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, updateTerminalTheme, UpdateTerminalThemeVariables } from '@dataconnect/generated';

// The `UpdateTerminalTheme` mutation requires an argument of type `UpdateTerminalThemeVariables`:
const updateTerminalThemeVars: UpdateTerminalThemeVariables = {
  id: ..., 
  opacity: ..., // optional
};

// Call the `updateTerminalTheme()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await updateTerminalTheme(updateTerminalThemeVars);
// Variables can be defined inline as well.
const { data } = await updateTerminalTheme({ id: ..., opacity: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await updateTerminalTheme(dataConnect, updateTerminalThemeVars);

console.log(data.terminalTheme_update);

// Or, you can use the `Promise` API.
updateTerminalTheme(updateTerminalThemeVars).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme_update);
});
```

### Using `UpdateTerminalTheme`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, updateTerminalThemeRef, UpdateTerminalThemeVariables } from '@dataconnect/generated';

// The `UpdateTerminalTheme` mutation requires an argument of type `UpdateTerminalThemeVariables`:
const updateTerminalThemeVars: UpdateTerminalThemeVariables = {
  id: ..., 
  opacity: ..., // optional
};

// Call the `updateTerminalThemeRef()` function to get a reference to the mutation.
const ref = updateTerminalThemeRef(updateTerminalThemeVars);
// Variables can be defined inline as well.
const ref = updateTerminalThemeRef({ id: ..., opacity: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = updateTerminalThemeRef(dataConnect, updateTerminalThemeVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.terminalTheme_update);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme_update);
});
```

## DeleteTerminalTheme
You can execute the `DeleteTerminalTheme` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
deleteTerminalTheme(vars: DeleteTerminalThemeVariables): MutationPromise<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;

interface DeleteTerminalThemeRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteTerminalThemeVariables): MutationRef<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;
}
export const deleteTerminalThemeRef: DeleteTerminalThemeRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
deleteTerminalTheme(dc: DataConnect, vars: DeleteTerminalThemeVariables): MutationPromise<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;

interface DeleteTerminalThemeRef {
  ...
  (dc: DataConnect, vars: DeleteTerminalThemeVariables): MutationRef<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;
}
export const deleteTerminalThemeRef: DeleteTerminalThemeRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the deleteTerminalThemeRef:
```typescript
const name = deleteTerminalThemeRef.operationName;
console.log(name);
```

### Variables
The `DeleteTerminalTheme` mutation requires an argument of type `DeleteTerminalThemeVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface DeleteTerminalThemeVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `DeleteTerminalTheme` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `DeleteTerminalThemeData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface DeleteTerminalThemeData {
  terminalTheme_delete?: TerminalTheme_Key | null;
}
```
### Using `DeleteTerminalTheme`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, deleteTerminalTheme, DeleteTerminalThemeVariables } from '@dataconnect/generated';

// The `DeleteTerminalTheme` mutation requires an argument of type `DeleteTerminalThemeVariables`:
const deleteTerminalThemeVars: DeleteTerminalThemeVariables = {
  id: ..., 
};

// Call the `deleteTerminalTheme()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await deleteTerminalTheme(deleteTerminalThemeVars);
// Variables can be defined inline as well.
const { data } = await deleteTerminalTheme({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await deleteTerminalTheme(dataConnect, deleteTerminalThemeVars);

console.log(data.terminalTheme_delete);

// Or, you can use the `Promise` API.
deleteTerminalTheme(deleteTerminalThemeVars).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme_delete);
});
```

### Using `DeleteTerminalTheme`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, deleteTerminalThemeRef, DeleteTerminalThemeVariables } from '@dataconnect/generated';

// The `DeleteTerminalTheme` mutation requires an argument of type `DeleteTerminalThemeVariables`:
const deleteTerminalThemeVars: DeleteTerminalThemeVariables = {
  id: ..., 
};

// Call the `deleteTerminalThemeRef()` function to get a reference to the mutation.
const ref = deleteTerminalThemeRef(deleteTerminalThemeVars);
// Variables can be defined inline as well.
const ref = deleteTerminalThemeRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = deleteTerminalThemeRef(dataConnect, deleteTerminalThemeVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.terminalTheme_delete);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.terminalTheme_delete);
});
```

## SyncDevice
You can execute the `SyncDevice` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
syncDevice(vars: SyncDeviceVariables): MutationPromise<SyncDeviceData, SyncDeviceVariables>;

interface SyncDeviceRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: SyncDeviceVariables): MutationRef<SyncDeviceData, SyncDeviceVariables>;
}
export const syncDeviceRef: SyncDeviceRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
syncDevice(dc: DataConnect, vars: SyncDeviceVariables): MutationPromise<SyncDeviceData, SyncDeviceVariables>;

interface SyncDeviceRef {
  ...
  (dc: DataConnect, vars: SyncDeviceVariables): MutationRef<SyncDeviceData, SyncDeviceVariables>;
}
export const syncDeviceRef: SyncDeviceRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the syncDeviceRef:
```typescript
const name = syncDeviceRef.operationName;
console.log(name);
```

### Variables
The `SyncDevice` mutation requires an argument of type `SyncDeviceVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface SyncDeviceVariables {
  name: string;
}
```
### Return Type
Recall that executing the `SyncDevice` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `SyncDeviceData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface SyncDeviceData {
  deviceSync_insert: DeviceSync_Key;
}
```
### Using `SyncDevice`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, syncDevice, SyncDeviceVariables } from '@dataconnect/generated';

// The `SyncDevice` mutation requires an argument of type `SyncDeviceVariables`:
const syncDeviceVars: SyncDeviceVariables = {
  name: ..., 
};

// Call the `syncDevice()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await syncDevice(syncDeviceVars);
// Variables can be defined inline as well.
const { data } = await syncDevice({ name: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await syncDevice(dataConnect, syncDeviceVars);

console.log(data.deviceSync_insert);

// Or, you can use the `Promise` API.
syncDevice(syncDeviceVars).then((response) => {
  const data = response.data;
  console.log(data.deviceSync_insert);
});
```

### Using `SyncDevice`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, syncDeviceRef, SyncDeviceVariables } from '@dataconnect/generated';

// The `SyncDevice` mutation requires an argument of type `SyncDeviceVariables`:
const syncDeviceVars: SyncDeviceVariables = {
  name: ..., 
};

// Call the `syncDeviceRef()` function to get a reference to the mutation.
const ref = syncDeviceRef(syncDeviceVars);
// Variables can be defined inline as well.
const ref = syncDeviceRef({ name: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = syncDeviceRef(dataConnect, syncDeviceVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.deviceSync_insert);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.deviceSync_insert);
});
```

## UpdateDeviceSync
You can execute the `UpdateDeviceSync` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
updateDeviceSync(vars: UpdateDeviceSyncVariables): MutationPromise<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;

interface UpdateDeviceSyncRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateDeviceSyncVariables): MutationRef<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;
}
export const updateDeviceSyncRef: UpdateDeviceSyncRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
updateDeviceSync(dc: DataConnect, vars: UpdateDeviceSyncVariables): MutationPromise<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;

interface UpdateDeviceSyncRef {
  ...
  (dc: DataConnect, vars: UpdateDeviceSyncVariables): MutationRef<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;
}
export const updateDeviceSyncRef: UpdateDeviceSyncRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the updateDeviceSyncRef:
```typescript
const name = updateDeviceSyncRef.operationName;
console.log(name);
```

### Variables
The `UpdateDeviceSync` mutation requires an argument of type `UpdateDeviceSyncVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface UpdateDeviceSyncVariables {
  id: UUIDString;
  profileId?: UUIDString | null;
}
```
### Return Type
Recall that executing the `UpdateDeviceSync` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `UpdateDeviceSyncData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface UpdateDeviceSyncData {
  deviceSync_update?: DeviceSync_Key | null;
}
```
### Using `UpdateDeviceSync`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, updateDeviceSync, UpdateDeviceSyncVariables } from '@dataconnect/generated';

// The `UpdateDeviceSync` mutation requires an argument of type `UpdateDeviceSyncVariables`:
const updateDeviceSyncVars: UpdateDeviceSyncVariables = {
  id: ..., 
  profileId: ..., // optional
};

// Call the `updateDeviceSync()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await updateDeviceSync(updateDeviceSyncVars);
// Variables can be defined inline as well.
const { data } = await updateDeviceSync({ id: ..., profileId: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await updateDeviceSync(dataConnect, updateDeviceSyncVars);

console.log(data.deviceSync_update);

// Or, you can use the `Promise` API.
updateDeviceSync(updateDeviceSyncVars).then((response) => {
  const data = response.data;
  console.log(data.deviceSync_update);
});
```

### Using `UpdateDeviceSync`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, updateDeviceSyncRef, UpdateDeviceSyncVariables } from '@dataconnect/generated';

// The `UpdateDeviceSync` mutation requires an argument of type `UpdateDeviceSyncVariables`:
const updateDeviceSyncVars: UpdateDeviceSyncVariables = {
  id: ..., 
  profileId: ..., // optional
};

// Call the `updateDeviceSyncRef()` function to get a reference to the mutation.
const ref = updateDeviceSyncRef(updateDeviceSyncVars);
// Variables can be defined inline as well.
const ref = updateDeviceSyncRef({ id: ..., profileId: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = updateDeviceSyncRef(dataConnect, updateDeviceSyncVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.deviceSync_update);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.deviceSync_update);
});
```

## DeleteDeviceSync
You can execute the `DeleteDeviceSync` mutation using the following action shortcut function, or by calling `executeMutation()` after calling the following `MutationRef` function, both of which are defined in [dataconnect-generated/index.d.ts](./index.d.ts):
```typescript
deleteDeviceSync(vars: DeleteDeviceSyncVariables): MutationPromise<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;

interface DeleteDeviceSyncRef {
  ...
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteDeviceSyncVariables): MutationRef<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;
}
export const deleteDeviceSyncRef: DeleteDeviceSyncRef;
```
You can also pass in a `DataConnect` instance to the action shortcut function or `MutationRef` function.
```typescript
deleteDeviceSync(dc: DataConnect, vars: DeleteDeviceSyncVariables): MutationPromise<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;

interface DeleteDeviceSyncRef {
  ...
  (dc: DataConnect, vars: DeleteDeviceSyncVariables): MutationRef<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;
}
export const deleteDeviceSyncRef: DeleteDeviceSyncRef;
```

If you need the name of the operation without creating a ref, you can retrieve the operation name by calling the `operationName` property on the deleteDeviceSyncRef:
```typescript
const name = deleteDeviceSyncRef.operationName;
console.log(name);
```

### Variables
The `DeleteDeviceSync` mutation requires an argument of type `DeleteDeviceSyncVariables`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:

```typescript
export interface DeleteDeviceSyncVariables {
  id: UUIDString;
}
```
### Return Type
Recall that executing the `DeleteDeviceSync` mutation returns a `MutationPromise` that resolves to an object with a `data` property.

The `data` property is an object of type `DeleteDeviceSyncData`, which is defined in [dataconnect-generated/index.d.ts](./index.d.ts). It has the following fields:
```typescript
export interface DeleteDeviceSyncData {
  deviceSync_delete?: DeviceSync_Key | null;
}
```
### Using `DeleteDeviceSync`'s action shortcut function

```typescript
import { getDataConnect } from 'firebase/data-connect';
import { connectorConfig, deleteDeviceSync, DeleteDeviceSyncVariables } from '@dataconnect/generated';

// The `DeleteDeviceSync` mutation requires an argument of type `DeleteDeviceSyncVariables`:
const deleteDeviceSyncVars: DeleteDeviceSyncVariables = {
  id: ..., 
};

// Call the `deleteDeviceSync()` function to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await deleteDeviceSync(deleteDeviceSyncVars);
// Variables can be defined inline as well.
const { data } = await deleteDeviceSync({ id: ..., });

// You can also pass in a `DataConnect` instance to the action shortcut function.
const dataConnect = getDataConnect(connectorConfig);
const { data } = await deleteDeviceSync(dataConnect, deleteDeviceSyncVars);

console.log(data.deviceSync_delete);

// Or, you can use the `Promise` API.
deleteDeviceSync(deleteDeviceSyncVars).then((response) => {
  const data = response.data;
  console.log(data.deviceSync_delete);
});
```

### Using `DeleteDeviceSync`'s `MutationRef` function

```typescript
import { getDataConnect, executeMutation } from 'firebase/data-connect';
import { connectorConfig, deleteDeviceSyncRef, DeleteDeviceSyncVariables } from '@dataconnect/generated';

// The `DeleteDeviceSync` mutation requires an argument of type `DeleteDeviceSyncVariables`:
const deleteDeviceSyncVars: DeleteDeviceSyncVariables = {
  id: ..., 
};

// Call the `deleteDeviceSyncRef()` function to get a reference to the mutation.
const ref = deleteDeviceSyncRef(deleteDeviceSyncVars);
// Variables can be defined inline as well.
const ref = deleteDeviceSyncRef({ id: ..., });

// You can also pass in a `DataConnect` instance to the `MutationRef` function.
const dataConnect = getDataConnect(connectorConfig);
const ref = deleteDeviceSyncRef(dataConnect, deleteDeviceSyncVars);

// Call `executeMutation()` on the reference to execute the mutation.
// You can use the `await` keyword to wait for the promise to resolve.
const { data } = await executeMutation(ref);

console.log(data.deviceSync_delete);

// Or, you can use the `Promise` API.
executeMutation(ref).then((response) => {
  const data = response.data;
  console.log(data.deviceSync_delete);
});
```

