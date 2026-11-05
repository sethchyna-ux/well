# dataconnect_generated SDK

## Installation
```sh
flutter pub get firebase_data_connect
flutterfire configure
```
For more information, see [Flutter for Firebase installation documentation](https://firebase.google.com/docs/data-connect/flutter-sdk#use-core).

## Data Connect instance
Each connector creates a static class, with an instance of the `DataConnect` class that can be used to connect to your Data Connect backend and call operations.

### Connecting to the emulator

```dart
String host = 'localhost'; // or your host name
int port = 9399; // or your port number
ExampleConnector.instance.dataConnect.useDataConnectEmulator(host, port);
```

You can also call queries and mutations by using the connector class.
## Queries

### GetCurrentUser
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.getCurrentUser().execute();
```



#### Return Type
`execute()` returns a `QueryResult<GetCurrentUserData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.getCurrentUser();
GetCurrentUserData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.getCurrentUser().ref();
ref.execute();

ref.subscribe(...);
```


### ListAllUsers
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.listAllUsers().execute();
```



#### Return Type
`execute()` returns a `QueryResult<ListAllUsersData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.listAllUsers();
ListAllUsersData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.listAllUsers().ref();
ref.execute();

ref.subscribe(...);
```


### GetProfile
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.getProfile(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `QueryResult<GetProfileData, GetProfileVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.getProfile(
  id: id,
);
GetProfileData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.getProfile(
  id: id,
).ref();
ref.execute();

ref.subscribe(...);
```


### ListMyProfiles
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.listMyProfiles().execute();
```



#### Return Type
`execute()` returns a `QueryResult<ListMyProfilesData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.listMyProfiles();
ListMyProfilesData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.listMyProfiles().ref();
ref.execute();

ref.subscribe(...);
```


### GetShellConfig
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.getShellConfig(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `QueryResult<GetShellConfigData, GetShellConfigVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.getShellConfig(
  id: id,
);
GetShellConfigData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.getShellConfig(
  id: id,
).ref();
ref.execute();

ref.subscribe(...);
```


### ListMyShellConfigs
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.listMyShellConfigs().execute();
```



#### Return Type
`execute()` returns a `QueryResult<ListMyShellConfigsData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.listMyShellConfigs();
ListMyShellConfigsData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.listMyShellConfigs().ref();
ref.execute();

ref.subscribe(...);
```


### GetCommandHistory
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.getCommandHistory(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `QueryResult<GetCommandHistoryData, GetCommandHistoryVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.getCommandHistory(
  id: id,
);
GetCommandHistoryData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.getCommandHistory(
  id: id,
).ref();
ref.execute();

ref.subscribe(...);
```


### ListMyCommandHistory
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.listMyCommandHistory().execute();
```



#### Return Type
`execute()` returns a `QueryResult<ListMyCommandHistoryData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.listMyCommandHistory();
ListMyCommandHistoryData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.listMyCommandHistory().ref();
ref.execute();

ref.subscribe(...);
```


### GetTerminalTheme
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.getTerminalTheme(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `QueryResult<GetTerminalThemeData, GetTerminalThemeVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.getTerminalTheme(
  id: id,
);
GetTerminalThemeData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.getTerminalTheme(
  id: id,
).ref();
ref.execute();

ref.subscribe(...);
```


### ListMyTerminalThemes
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.listMyTerminalThemes().execute();
```



#### Return Type
`execute()` returns a `QueryResult<ListMyTerminalThemesData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.listMyTerminalThemes();
ListMyTerminalThemesData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.listMyTerminalThemes().ref();
ref.execute();

ref.subscribe(...);
```


### GetDeviceSync
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.getDeviceSync(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `QueryResult<GetDeviceSyncData, GetDeviceSyncVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.getDeviceSync(
  id: id,
);
GetDeviceSyncData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.getDeviceSync(
  id: id,
).ref();
ref.execute();

ref.subscribe(...);
```


### ListMyDevices
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.listMyDevices().execute();
```



#### Return Type
`execute()` returns a `QueryResult<ListMyDevicesData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

/// Result of a query request. Created to hold extra variables in the future.
class QueryResult<Data, Variables> extends OperationResult<Data, Variables> {
  QueryResult(super.dataConnect, super.data, super.ref);
}

final result = await ExampleConnector.instance.listMyDevices();
ListMyDevicesData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.listMyDevices().ref();
ref.execute();

ref.subscribe(...);
```

## Mutations

### CreateUser
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.createUser().execute();
```



#### Return Type
`execute()` returns a `OperationResult<CreateUserData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.createUser();
CreateUserData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.createUser().ref();
ref.execute();
```


### UpdateUser
#### Required Arguments
```dart
String username = ...;
ExampleConnector.instance.updateUser(
  username: username,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<UpdateUserData, UpdateUserVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.updateUser(
  username: username,
);
UpdateUserData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String username = ...;

final ref = ExampleConnector.instance.updateUser(
  username: username,
).ref();
ref.execute();
```


### DeleteUser
#### Required Arguments
```dart
// No required arguments
ExampleConnector.instance.deleteUser().execute();
```



#### Return Type
`execute()` returns a `OperationResult<DeleteUserData, void>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.deleteUser();
DeleteUserData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
final ref = ExampleConnector.instance.deleteUser().ref();
ref.execute();
```


### CreateProfile
#### Required Arguments
```dart
String name = ...;
ExampleConnector.instance.createProfile(
  name: name,
).execute();
```

#### Optional Arguments
We return a builder for each query. For CreateProfile, we created `CreateProfileBuilder`. For queries and mutations with optional parameters, we return a builder class.
The builder pattern allows Data Connect to distinguish between fields that haven't been set and fields that have been set to null. A field can be set by calling its respective setter method like below:
```dart
class CreateProfileVariablesBuilder {
  ...
   CreateProfileVariablesBuilder metadata(String? t) {
   _metadata.value = t;
   return this;
  }

  ...
}
ExampleConnector.instance.createProfile(
  name: name,
)
.metadata(metadata)
.execute();
```

#### Return Type
`execute()` returns a `OperationResult<CreateProfileData, CreateProfileVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.createProfile(
  name: name,
);
CreateProfileData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String name = ...;

final ref = ExampleConnector.instance.createProfile(
  name: name,
).ref();
ref.execute();
```


### UpdateProfile
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.updateProfile(
  id: id,
).execute();
```

#### Optional Arguments
We return a builder for each query. For UpdateProfile, we created `UpdateProfileBuilder`. For queries and mutations with optional parameters, we return a builder class.
The builder pattern allows Data Connect to distinguish between fields that haven't been set and fields that have been set to null. A field can be set by calling its respective setter method like below:
```dart
class UpdateProfileVariablesBuilder {
  ...
   UpdateProfileVariablesBuilder name(String? t) {
   _name.value = t;
   return this;
  }

  ...
}
ExampleConnector.instance.updateProfile(
  id: id,
)
.name(name)
.execute();
```

#### Return Type
`execute()` returns a `OperationResult<UpdateProfileData, UpdateProfileVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.updateProfile(
  id: id,
);
UpdateProfileData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.updateProfile(
  id: id,
).ref();
ref.execute();
```


### DeleteProfile
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.deleteProfile(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<DeleteProfileData, DeleteProfileVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.deleteProfile(
  id: id,
);
DeleteProfileData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.deleteProfile(
  id: id,
).ref();
ref.execute();
```


### CreateShellConfig
#### Required Arguments
```dart
String profileId = ...;
String prompt = ...;
String alias = ...;
ExampleConnector.instance.createShellConfig(
  profileId: profileId,
  prompt: prompt,
  alias: alias,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<CreateShellConfigData, CreateShellConfigVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.createShellConfig(
  profileId: profileId,
  prompt: prompt,
  alias: alias,
);
CreateShellConfigData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String profileId = ...;
String prompt = ...;
String alias = ...;

final ref = ExampleConnector.instance.createShellConfig(
  profileId: profileId,
  prompt: prompt,
  alias: alias,
).ref();
ref.execute();
```


### UpdateShellConfig
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.updateShellConfig(
  id: id,
).execute();
```

#### Optional Arguments
We return a builder for each query. For UpdateShellConfig, we created `UpdateShellConfigBuilder`. For queries and mutations with optional parameters, we return a builder class.
The builder pattern allows Data Connect to distinguish between fields that haven't been set and fields that have been set to null. A field can be set by calling its respective setter method like below:
```dart
class UpdateShellConfigVariablesBuilder {
  ...
   UpdateShellConfigVariablesBuilder prompt(String? t) {
   _prompt.value = t;
   return this;
  }

  ...
}
ExampleConnector.instance.updateShellConfig(
  id: id,
)
.prompt(prompt)
.execute();
```

#### Return Type
`execute()` returns a `OperationResult<UpdateShellConfigData, UpdateShellConfigVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.updateShellConfig(
  id: id,
);
UpdateShellConfigData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.updateShellConfig(
  id: id,
).ref();
ref.execute();
```


### DeleteShellConfig
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.deleteShellConfig(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<DeleteShellConfigData, DeleteShellConfigVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.deleteShellConfig(
  id: id,
);
DeleteShellConfigData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.deleteShellConfig(
  id: id,
).ref();
ref.execute();
```


### AddCommandHistory
#### Required Arguments
```dart
String cmd = ...;
ExampleConnector.instance.addCommandHistory(
  cmd: cmd,
).execute();
```

#### Optional Arguments
We return a builder for each query. For AddCommandHistory, we created `AddCommandHistoryBuilder`. For queries and mutations with optional parameters, we return a builder class.
The builder pattern allows Data Connect to distinguish between fields that haven't been set and fields that have been set to null. A field can be set by calling its respective setter method like below:
```dart
class AddCommandHistoryVariablesBuilder {
  ...
   AddCommandHistoryVariablesBuilder tag(String? t) {
   _tag.value = t;
   return this;
  }

  ...
}
ExampleConnector.instance.addCommandHistory(
  cmd: cmd,
)
.tag(tag)
.execute();
```

#### Return Type
`execute()` returns a `OperationResult<AddCommandHistoryData, AddCommandHistoryVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.addCommandHistory(
  cmd: cmd,
);
AddCommandHistoryData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String cmd = ...;

final ref = ExampleConnector.instance.addCommandHistory(
  cmd: cmd,
).ref();
ref.execute();
```


### DeleteCommandHistory
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.deleteCommandHistory(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<DeleteCommandHistoryData, DeleteCommandHistoryVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.deleteCommandHistory(
  id: id,
);
DeleteCommandHistoryData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.deleteCommandHistory(
  id: id,
).ref();
ref.execute();
```


### CreateTerminalTheme
#### Required Arguments
```dart
String name = ...;
String palette = ...;
String profileId = ...;
ExampleConnector.instance.createTerminalTheme(
  name: name,
  palette: palette,
  profileId: profileId,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<CreateTerminalThemeData, CreateTerminalThemeVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.createTerminalTheme(
  name: name,
  palette: palette,
  profileId: profileId,
);
CreateTerminalThemeData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String name = ...;
String palette = ...;
String profileId = ...;

final ref = ExampleConnector.instance.createTerminalTheme(
  name: name,
  palette: palette,
  profileId: profileId,
).ref();
ref.execute();
```


### UpdateTerminalTheme
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.updateTerminalTheme(
  id: id,
).execute();
```

#### Optional Arguments
We return a builder for each query. For UpdateTerminalTheme, we created `UpdateTerminalThemeBuilder`. For queries and mutations with optional parameters, we return a builder class.
The builder pattern allows Data Connect to distinguish between fields that haven't been set and fields that have been set to null. A field can be set by calling its respective setter method like below:
```dart
class UpdateTerminalThemeVariablesBuilder {
  ...
   UpdateTerminalThemeVariablesBuilder opacity(double? t) {
   _opacity.value = t;
   return this;
  }

  ...
}
ExampleConnector.instance.updateTerminalTheme(
  id: id,
)
.opacity(opacity)
.execute();
```

#### Return Type
`execute()` returns a `OperationResult<UpdateTerminalThemeData, UpdateTerminalThemeVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.updateTerminalTheme(
  id: id,
);
UpdateTerminalThemeData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.updateTerminalTheme(
  id: id,
).ref();
ref.execute();
```


### DeleteTerminalTheme
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.deleteTerminalTheme(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<DeleteTerminalThemeData, DeleteTerminalThemeVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.deleteTerminalTheme(
  id: id,
);
DeleteTerminalThemeData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.deleteTerminalTheme(
  id: id,
).ref();
ref.execute();
```


### SyncDevice
#### Required Arguments
```dart
String name = ...;
ExampleConnector.instance.syncDevice(
  name: name,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<SyncDeviceData, SyncDeviceVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.syncDevice(
  name: name,
);
SyncDeviceData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String name = ...;

final ref = ExampleConnector.instance.syncDevice(
  name: name,
).ref();
ref.execute();
```


### UpdateDeviceSync
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.updateDeviceSync(
  id: id,
).execute();
```

#### Optional Arguments
We return a builder for each query. For UpdateDeviceSync, we created `UpdateDeviceSyncBuilder`. For queries and mutations with optional parameters, we return a builder class.
The builder pattern allows Data Connect to distinguish between fields that haven't been set and fields that have been set to null. A field can be set by calling its respective setter method like below:
```dart
class UpdateDeviceSyncVariablesBuilder {
  ...
   UpdateDeviceSyncVariablesBuilder profileId(String? t) {
   _profileId.value = t;
   return this;
  }

  ...
}
ExampleConnector.instance.updateDeviceSync(
  id: id,
)
.profileId(profileId)
.execute();
```

#### Return Type
`execute()` returns a `OperationResult<UpdateDeviceSyncData, UpdateDeviceSyncVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.updateDeviceSync(
  id: id,
);
UpdateDeviceSyncData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.updateDeviceSync(
  id: id,
).ref();
ref.execute();
```


### DeleteDeviceSync
#### Required Arguments
```dart
String id = ...;
ExampleConnector.instance.deleteDeviceSync(
  id: id,
).execute();
```



#### Return Type
`execute()` returns a `OperationResult<DeleteDeviceSyncData, DeleteDeviceSyncVariables>`
```dart
/// Result of an Operation Request (query/mutation).
class OperationResult<Data, Variables> {
  OperationResult(this.dataConnect, this.data, this.ref);
  Data data;
  OperationRef<Data, Variables> ref;
  FirebaseDataConnect dataConnect;
}

final result = await ExampleConnector.instance.deleteDeviceSync(
  id: id,
);
DeleteDeviceSyncData data = result.data;
final ref = result.ref;
```

#### Getting the Ref
Each builder returns an `execute` function, which is a helper function that creates a `Ref` object, and executes the underlying operation.
An example of how to use the `Ref` object is shown below:
```dart
String id = ...;

final ref = ExampleConnector.instance.deleteDeviceSync(
  id: id,
).ref();
ref.execute();
```

